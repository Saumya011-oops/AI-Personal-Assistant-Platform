use std::collections::{HashMap, HashSet};

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::db::Database;
use crate::domain::{
    AssistantResponse, Citation, ConfidenceBreakdown, DiagChunk, DiagnosticsPayload,
    MetadataFilters, QueryAnalysis, RetrievalResponse, RetrievalStrategy,
    RetrievedChunk,
};
use crate::services::context_builder::{BuiltContext, ContextBuilder};
use crate::services::groq::GroqService;
use crate::services::ollama::OllamaService;
use crate::services::qdrant::{QdrantSearchFilter, QdrantSearchResult, QdrantService};
use crate::services::query_analyzer::QueryAnalyzerService;
use crate::services::reranker::RerankerService;
use crate::services::sparse::SparseRetrievalService;

#[derive(Clone)]
pub struct RetrievalService {
    ollama_service: OllamaService,
    qdrant_service: QdrantService,
    sparse_service: SparseRetrievalService,
    groq_service: GroqService,
    query_analyzer_service: QueryAnalyzerService,
    reranker_service: RerankerService,
    context_builder: ContextBuilder,
}

impl RetrievalService {
    pub fn new(
        ollama_service: OllamaService,
        qdrant_service: QdrantService,
        sparse_service: SparseRetrievalService,
        groq_service: GroqService,
        query_analyzer_service: QueryAnalyzerService,
        reranker_service: RerankerService,
        context_builder: ContextBuilder,
    ) -> Self {
        Self {
            ollama_service,
            qdrant_service,
            sparse_service,
            groq_service,
            query_analyzer_service,
            reranker_service,
            context_builder,
        }
    }

    pub async fn ask_assistant(&self, database: &Database, query: &str) -> Result<AssistantResponse> {
        // ── RAG_DEBUG_MODE: bypass all confidence gating for diagnosis ─────────
        // Set RAG_DEBUG_MODE=true in the environment to always answer using retrieved
        // context regardless of confidence/ambiguity status. Use this to determine
        // whether failures are in retrieval vs the confidence layer.
        let debug_mode = std::env::var("RAG_DEBUG_MODE")
            .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
            .unwrap_or(false);
        if debug_mode {
            tracing::warn!("[DEBUG_MODE] RAG_DEBUG_MODE=true — confidence/ambiguity gating DISABLED");
        }

        let retrieval_start = std::time::Instant::now();
        let retrieval = self.retrieve_documents(database, query).await?;
        let retrieval_latency_ms = retrieval_start.elapsed().as_millis() as u32;

        let mut retrieval_results = retrieval.results.clone();
        for chunk in &mut retrieval_results {
            chunk.retrieval_score = Some(chunk.score);
        }

        // ── Expanded query (for diagnostics payload) ───────────────────────────
        let expanded_query = expand_query(query);

        // Apply metadata boosting for configuration-style queries only
        if is_configuration_query(query) {
            boost_configuration_chunks(&mut retrieval_results);
        }

        // ── Pre-rerank snapshot (recall evaluation checkpoint 1) ───────────────
        let pre_rerank_chunks = retrieval_results.clone();

        let rerank_start = std::time::Instant::now();
        let mut reranked_chunks = self
            .reranker_service
            .rerank(query, retrieval_results)
            .await?;
        let rerank_latency_ms = rerank_start.elapsed().as_millis() as u32;

        // Re-boost and sort reranked chunks if config query to ensure boosted matches remain on top
        if is_configuration_query(query) {
            boost_configuration_chunks(&mut reranked_chunks);
            sort_descending(&mut reranked_chunks);
        }

        // ── Post-rerank snapshot (recall evaluation checkpoint 2) ──────────────
        let post_rerank_chunks = reranked_chunks.clone();

        // ── [RERANKER] structured score log (raw logit + sigmoid for calibration)
        // Model: cross-encoder/ms-marco-MiniLM-L-6-v2 → raw logits, NOT probabilities.
        // Correct matches typically score in [-3, +4]. sigmoid(-2)=0.119, sigmoid(-1)=0.269.
        {
            let sigmoid = |x: f32| 1.0_f32 / (1.0 + (-x).exp());
            for (rank, chunk) in reranked_chunks.iter().take(10).enumerate() {
                tracing::info!(
                    "[RERANKER] query={:?} rank={} doc={:?} raw_logit={:.4} sigmoid={:.4}",
                    query, rank + 1, chunk.document_title, chunk.score, sigmoid(chunk.score)
                );
            }
        }

        // ── Confidence calculation ─────────────────────────────────────────────
        let (confidence_report, confidence_breakdown) = self.calculate_confidence_full(
            query,
            &reranked_chunks,
            &retrieval.strategy_used,
            &retrieval.analysis.complexity,
        );

        tracing::info!(
            "Retrieval Confidence Status: {}, Score: {}, Reasons: {:?}",
            confidence_report.status,
            confidence_report.confidence_score,
            confidence_report.reasons
        );

        // Document Aggregation
        let doc_aggregations = aggregate_documents(&reranked_chunks);
        let top_document = doc_aggregations.first().map(|d| d.document_title.as_str());
        let top_document_score = doc_aggregations.first().map(|d| d.document_score);
        let ambiguity_score = confidence_report.ambiguity_score;

        // Hop count calculation
        let hop_count = reranked_chunks.iter()
            .filter_map(|c| c.metadata.get("lineage"))
            .filter_map(|l| l.get("hop_number"))
            .filter_map(|h| h.as_u64())
            .max()
            .unwrap_or(1) as u32;

        // Lineage array builder
        let mut lineages = Vec::new();
        for chunk in &reranked_chunks {
            if let Some(lineage) = chunk.metadata.get("lineage") {
                lineages.push(json!({
                    "chunk_id": chunk.chunk_id,
                    "lineage": lineage
                }));
            }
        }
        let lineage_json = if lineages.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&lineages)?)
        };

        // ── Main answer / bypass logic ─────────────────────────────────────────
        // When RAG_DEBUG_MODE=true, skip all confidence/ambiguity gating and always
        // attempt LLM generation. This lets us isolate retrieval failures from
        // confidence-layer failures. The debug answer includes the confidence status
        // so you can see what would normally have been gated.
        let (answer, citations, fact_coverage) = if !debug_mode && confidence_report.status == "EMPTY_RETRIEVAL" {
            let fallback_answer = format!(
                "I could not find relevant information in the connected knowledge base.\n\nStatus: EMPTY_RETRIEVAL\nConfidence: {}/100\n\nReasons:\n{}",
                confidence_report.confidence_score,
                confidence_report.reasons.iter().map(|r| format!("- {}", r)).collect::<Vec<_>>().join("\n")
            );
            (fallback_answer, Vec::new(), 1.0_f32)
        } else if !debug_mode && confidence_report.status == "LOW_CONFIDENCE_RETRIEVAL" {
            let sources = doc_aggregations.iter()
                .map(|d| format!("- {} ({})", d.document_title, d.source))
                .collect::<Vec<_>>()
                .join("\n");
            let fallback_answer = format!(
                "I found potentially relevant information but confidence is low.\n\nStatus: LOW_CONFIDENCE_RETRIEVAL\nConfidence: {}/100\n\nRetrieved Sources:\n{}\n\nPlease refine your query to be more specific or add more context.",
                confidence_report.confidence_score,
                sources
            );
            (fallback_answer, Vec::new(), 1.0_f32)
        } else if !debug_mode && confidence_report.status == "AMBIGUOUS_RETRIEVAL" {
            let stop_words: HashSet<&str> = [
                "what", "is", "the", "does", "how", "to", "explain", "describe", "about",
                "system", "use", "connect", "setup", "integration", "with", "and", "or",
                "a", "an", "of", "which", "whose", "where", "whom", "related"
            ].iter().cloned().collect();

            let subject = query.to_lowercase()
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty() && !stop_words.contains(s) && s.len() > 3)
                .next()
                .map(|s| clean_document_title(s))
                .unwrap_or_else(|| clean_document_title(query));

            let mut fallback_answer = format!(
                "I found multiple {}-related topics.\n\n",
                subject
            );
            for (i, doc) in doc_aggregations.iter().take(3).enumerate() {
                fallback_answer.push_str(&format!("{}. {}\n", i + 1, clean_document_title(&doc.document_title)));
            }
            fallback_answer.push_str("\nWhich would you like me to explain?");
            (fallback_answer, Vec::new(), 1.0_f32)
        } else {
            // PARTIAL_RETRIEVAL or OK
            let mut top_chunks = reranked_chunks.clone();
            top_chunks.truncate(3);

            let built_context = self.context_builder.build(top_chunks.clone());
            let mut ans = self.generate_answer(query, &retrieval.analysis, &built_context).await?;

            // ── Fact-level citation verification ──────────────────────────────
            let coverage = verify_answer_against_chunks(&ans, &top_chunks);
            if coverage < 0.50 {
                ans = format!(
                    "⚠️ Some claims in this answer could not be traced to retrieved sources.\n\n{}",
                    ans
                );
                // Soft downgrade: OK → PARTIAL, PARTIAL → LOW (do not touch EMPTY/LOW)
            }

            let cits: Vec<Citation> = built_context
                .chunks
                .into_iter()
                .map(|chunk| {
                    let matching = top_chunks.iter().find(|c| c.chunk_id == chunk.chunk_id);
                    let retrieval_score = matching.and_then(|c| c.retrieval_score);
                    let rerank_score = matching.map(|c| c.score).unwrap_or(chunk.score);

                    Citation {
                        source_document: matching.map(|c| c.document_title.clone()).unwrap_or_else(|| chunk.source.clone()),
                        source_type: chunk.source.clone(),
                        chunk_id: chunk.chunk_id.clone(),
                        retrieval_score,
                        rerank_score,
                        // Legacy:
                        source: chunk.source.clone(),
                        document_id: chunk.document_id.clone(),
                        score: chunk.score,
                    }
                })
                .collect();

            // Prepend warning if PARTIAL_RETRIEVAL
            if confidence_report.status == "PARTIAL_RETRIEVAL" {
                ans = format!(
                    "⚠️ This answer may be incomplete because only part of the requested information was found.\n\n{}",
                    ans
                );
            }

            // Append citations block to answer
            let mut formatted_citations = String::new();
            if !cits.is_empty() {
                formatted_citations.push_str("\n\nSources:\n");
                for cit in &cits {
                    let section = reranked_chunks.iter()
                        .find(|c| c.chunk_id == cit.chunk_id)
                        .and_then(|c| c.metadata.get("section"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("General");

                    let ordinal = reranked_chunks.iter()
                        .find(|c| c.chunk_id == cit.chunk_id)
                        .map(|c| c.ordinal)
                        .unwrap_or(0);

                    formatted_citations.push_str(&format!(
                        "Source:\n{}\nSection:\n{}\nChunk:\n{}\nScore:\n{:.2}\n\n",
                        cit.source_document,
                        section,
                        ordinal,
                        cit.rerank_score
                    ));
                }
            }
            ans.push_str(&formatted_citations);
            (ans, cits, coverage)
        };

        // ── Recall Metrics ─────────────────────────────────────────────────────
        let recall = compute_recall_metrics(&pre_rerank_chunks, &post_rerank_chunks, fact_coverage);

        // ── Structured [RECALL] log — separates retrieval / confidence / LLM failures
        tracing::info!(
            query = %query,
            strategy = ?retrieval.strategy_used,
            pre_rerank_top_doc = %pre_rerank_chunks.first().map(|c| c.document_title.as_str()).unwrap_or("none"),
            pre_rerank_unique_docs = %recall.unique_docs_pre_rerank,
            post_rerank_top_doc = %post_rerank_chunks.first().map(|c| c.document_title.as_str()).unwrap_or("none"),
            post_rerank_unique_docs = %recall.unique_docs_post_rerank,
            top_doc_changed_by_reranker = %recall.top_doc_changed,
            fact_coverage = %recall.fact_coverage,
            confidence_status = %confidence_report.status,
            confidence_score = %confidence_report.confidence_score,
            "[RECALL]"
        );

        // ── Persist telemetry ──────────────────────────────────────────────────
        if let Err(err) = database.document_repository().save_rag_telemetry(
            query,
            &format!("{:?}", retrieval.strategy_used),
            confidence_report.confidence_score,
            &confidence_report.status,
            &confidence_report.reasons,
            &confidence_report.confidence,
            top_document,
            top_document_score,
            ambiguity_score,
            hop_count,
            retrieval_latency_ms,
            rerank_latency_ms,
            lineage_json.as_deref(),
            pre_rerank_chunks.first().map(|c| c.document_title.as_str()),
            post_rerank_chunks.first().map(|c| c.document_title.as_str()),
            recall.unique_docs_pre_rerank as u32,
            recall.unique_docs_post_rerank as u32,
            recall.top_doc_changed,
            fact_coverage,
        ) {
            tracing::error!("Failed to persist RAG telemetry to SQLite: {}", err);
        }

        // ── Build diagnostics payload ──────────────────────────────────────────
        let diag_pre: Vec<DiagChunk> = pre_rerank_chunks.iter().take(10).map(|c| DiagChunk {
            chunk_id: c.chunk_id.clone(),
            document_title: c.document_title.clone(),
            retrieval_score: c.retrieval_score.unwrap_or(c.score),
            rerank_score: 0.0, // not yet reranked
        }).collect();

        let diag_post: Vec<DiagChunk> = post_rerank_chunks.iter().take(10).map(|c| DiagChunk {
            chunk_id: c.chunk_id.clone(),
            document_title: c.document_title.clone(),
            retrieval_score: c.retrieval_score.unwrap_or(0.0),
            rerank_score: c.score,
        }).collect();

        let diagnostics = DiagnosticsPayload {
            strategy: format!("{:?}", retrieval.strategy_used),
            query_expanded: expanded_query,
            pre_rerank_chunks: diag_pre,
            post_rerank_chunks: diag_post,
            confidence_breakdown,
            final_status: confidence_report.status.clone(),
            recall_metrics: recall,
        };

        Ok(AssistantResponse {
            answer,
            citations,
            confidence: Some(confidence_report),
            diagnostics: Some(diagnostics),
        })
    }

    /// Full confidence calculation returning both the public ConfidenceReport
    /// and the internal ConfidenceBreakdown needed for the diagnostics payload.
    pub fn calculate_confidence_full(
        &self,
        query: &str,
        chunks: &[RetrievedChunk],
        strategy: &RetrievalStrategy,
        complexity: &crate::domain::QueryComplexity,
    ) -> (crate::domain::ConfidenceReport, ConfidenceBreakdown) {
        let mut reasons = Vec::new();

        if chunks.is_empty() {
            let breakdown = ConfidenceBreakdown {
                reranker_top_sigmoid: 0.0,
                avg_top5_sigmoid: 0.0,
                chunk_count_bonus: 0,
                document_focus_bonus: 0,
                keyword_overlap_score: 0.0,
                retrieval_signal_score: 0.0,
                final_score: 0,
                status: "EMPTY_RETRIEVAL".to_string(),
            };
            return (
                crate::domain::ConfidenceReport {
                    confidence: "low".to_string(),
                    confidence_score: 0,
                    reasons: vec!["No relevant chunks retrieved.".to_string()],
                    status: "EMPTY_RETRIEVAL".to_string(),
                    ambiguity_score: Some(0.0),
                },
                breakdown,
            );
        }

        let sigmoid = |x: f32| -> f32 { 1.0 / (1.0 + (-x).exp()) };

        // ── Signal 1 & 2: Reranker scores (35% + 15% = 50%) ─────────────────────
        // Model: cross-encoder/ms-marco-MiniLM-L-6-v2 → raw logits in [-10, +10].
        // Correct matches: typically [-3, +4]. sigmoid(-2)=0.119, sigmoid(-1)=0.269.
        let top_rerank_score = chunks[0].score;
        let top_normalized_rerank = sigmoid(top_rerank_score);

        let limit = chunks.len().min(5);
        let sum_top_5: f32 = chunks.iter().take(limit).map(|c| sigmoid(c.score)).sum();
        let avg_top_5_normalized_rerank = sum_top_5 / limit as f32;

        // Variance across all reranked chunks
        let mean_all: f32 = chunks.iter().map(|c| sigmoid(c.score)).sum::<f32>() / chunks.len() as f32;
        let variance_all: f32 = chunks.iter().map(|c| {
            let diff = sigmoid(c.score) - mean_all;
            diff * diff
        }).sum::<f32>() / chunks.len() as f32;

        // ── Signal 3: Chunk count bonus (15%) ────────────────────────────────────
        let chunk_count_bonus = (chunks.len().min(3) as i32) * 5; // 0, 5, 10, or 15

        // ── Signal 4: Document focus bonus (15%) ─────────────────────────────────
        let doc_aggregations = aggregate_documents(chunks);
        let unique_docs = doc_aggregations.len();
        let document_focus_bonus: i32 = if unique_docs == 1 && chunks.len() >= 3 {
            15
        } else if unique_docs <= 2 {
            8
        } else {
            0
        };

        // ── Signal 5: Keyword overlap (10%) ──────────────────────────────────────
        let kw_overlap = keyword_overlap_score(query, chunks);
        let kw_bonus = (kw_overlap * 10.0) as i32;

        // ── Signal 6: Retrieval signal score (10%) ────────────────────────────────
        let top_retrieval_score = chunks.iter()
            .filter_map(|c| c.retrieval_score)
            .next()
            .unwrap_or(0.0);
        let retrieval_signal_score: f32 = match strategy {
            RetrievalStrategy::Dense => {
                if top_retrieval_score >= 0.6 { 10.0 } else if top_retrieval_score < 0.45 { 0.0 } else { 5.0 }
            }
            RetrievalStrategy::Sparse => {
                if top_retrieval_score <= -2.0 { 10.0 } else if top_retrieval_score > -1.0 { 0.0 } else { 5.0 }
            }
            _ => {
                if top_retrieval_score >= 0.015 { 10.0 } else if top_retrieval_score < 0.008 { 0.0 } else { 5.0 }
            }
        };

        // ── Signal 7: Title keyword boost (+15) ───────────────────────────────────
        // If the query's primary noun appears verbatim in the top document title,
        // this is a very strong relevance signal (direct name match).
        let title_keyword_boost = query_title_match_bonus(query, chunks);

        // ── Combine signals ───────────────────────────────────────────────────────
        // Reranker: 35% top + 15% avg = 50 points max
        // Chunk count: up to 15 points
        // Document focus: up to 15 points
        // Keyword overlap: up to 10 points
        // Retrieval signal: up to 10 points
        // Title keyword bonus: +15 (strong direct-match signal)
        let mut confidence_score =
            (top_normalized_rerank * 35.0) as i32
            + (avg_top_5_normalized_rerank * 15.0) as i32
            + chunk_count_bonus
            + document_focus_bonus
            + kw_bonus
            + retrieval_signal_score as i32
            + title_keyword_boost;

        // ── Reason messages ───────────────────────────────────────────────────────
        if top_normalized_rerank >= 0.80 {
            reasons.push("Strong relevance match identified by reranker.".to_string());
        } else if top_normalized_rerank < 0.40 {
            reasons.push("Reranker indicates low relevance matching.".to_string());
        }
        if kw_overlap >= 0.4 {
            reasons.push("High keyword overlap between query and retrieved chunks.".to_string());
        }
        if document_focus_bonus >= 15 {
            reasons.push("High document focus (all chunks from single source).".to_string());
        }
        if title_keyword_boost > 0 {
            reasons.push("Query noun matched top document title directly.".to_string());
        }
        if unique_docs > 4 {
            if matches!(complexity, crate::domain::QueryComplexity::Complex) {
                confidence_score -= 5;
                reasons.push("Spans multiple documents for a complex query.".to_string());
            } else {
                confidence_score -= 10;
                reasons.push("High document fragmentation across multiple documents.".to_string());
            }
        }
        if variance_all < 0.005 && chunks.len() >= 4 {
            confidence_score -= 5;
            reasons.push("Flat reranker score distribution indicates uncertainty.".to_string());
        }

        // ── Ambiguity check ───────────────────────────────────────────────────────
        let ambiguity_score = if unique_docs > 1 {
            let top_doc_score = sigmoid(doc_aggregations[0].document_score);
            let second_doc_score = sigmoid(doc_aggregations[1].document_score);
            1.0 - (top_doc_score - second_doc_score).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let is_ambiguous = is_ambiguous_retrieval(query, chunks, strategy);
        if is_ambiguous {
            confidence_score -= 10;
            reasons.push("Ambiguous relevance — multiple topic clusters detected.".to_string());
        }

        let confidence_score = confidence_score.clamp(0, 100) as u32;

        // ── Status thresholds — calibrated for ms-marco-MiniLM-L-6-v2 logit output ─
        //
        // Logit-to-sigmoid mapping for ms-marco-MiniLM-L-6-v2:
        //   Correct match logit range: roughly [-3, +4]
        //   sigmoid(-3.00) = 0.047  ← near-zero relevance
        //   sigmoid(-2.59) = 0.070  ← EMPTY threshold (new)
        //   sigmoid(-1.73) = 0.150  ← LOW threshold (new)
        //   sigmoid(-0.84) = 0.300  ← PARTIAL threshold (new)
        //   sigmoid( 0.00) = 0.500  ← was the old PARTIAL threshold (too strict)
        //
        // Old thresholds were: empty=0.05, low=0.30, partial=0.50
        // These caused valid docs scoring logit=-2 to -1 to be classified LOW or PARTIAL.
        let (empty_threshold, low_threshold, partial_threshold) = if strategy == &RetrievalStrategy::Recursive {
            (0.05, 0.08, 0.20)
        } else {
            (0.07, 0.12, 0.30)
        };

        let status = if top_normalized_rerank < empty_threshold && kw_overlap == 0.0 {
            // Only EMPTY when sigmoid is near-zero AND there's zero keyword overlap.
            // A logit of -2.59 corresponds to sigmoid=0.07; anything above is potentially relevant.
            reasons.push("No chunks found or all chunk scores are below minimum matching relevance.".to_string());
            "EMPTY_RETRIEVAL".to_string()
        } else if is_ambiguous {
            reasons.push("Multiple documents competing across distinct topic clusters.".to_string());
            "AMBIGUOUS_RETRIEVAL".to_string()
        } else if top_normalized_rerank < low_threshold && kw_overlap < 0.05 && chunks.len() < 2 {
            // LOW: weak logit AND near-zero keyword overlap AND only 1 chunk returned.
            // Previously fired at sigmoid < 0.30 which incorrectly caught valid logit=-1 docs.
            reasons.push("Overall low matching confidence across retrieval layers.".to_string());
            "LOW_CONFIDENCE_RETRIEVAL".to_string()
        } else if top_normalized_rerank < partial_threshold {
            // PARTIAL: sigmoid 0.07–0.30, meaning logit -2.59 to -0.84.
            // These docs ARE retrieved and likely relevant but with moderate confidence.
            reasons.push("Partial context retrieved; some sources may be weakly relevant.".to_string());
            "PARTIAL_RETRIEVAL".to_string()
        } else {
            "OK".to_string()
        };

        // ── [CONFIDENCE] structured log for failure attribution ───────────────────
        tracing::info!(
            "[CONFIDENCE] query={:?} status={} top_logit={:.4} top_sigmoid={:.4} kw_overlap={:.4} title_boost={} ambiguity_fired={} score_gap={:.4} final_score={}",
            query, status, top_rerank_score, top_normalized_rerank, kw_overlap, title_keyword_boost,
            is_ambiguous,
            if unique_docs > 1 {
                let sigmoid_doc1 = sigmoid(doc_aggregations[0].document_score);
                let sigmoid_doc2 = sigmoid(doc_aggregations[1].document_score);
                sigmoid_doc1 - sigmoid_doc2
            } else { 1.0 },
            confidence_score
        );

        let confidence = if confidence_score >= 75 {
            "high".to_string()
        } else if confidence_score >= 40 {
            "medium".to_string()
        } else {
            "low".to_string()
        };

        let breakdown = ConfidenceBreakdown {
            reranker_top_sigmoid: top_normalized_rerank,
            avg_top5_sigmoid: avg_top_5_normalized_rerank,
            chunk_count_bonus,
            document_focus_bonus,
            keyword_overlap_score: kw_overlap,
            retrieval_signal_score,
            final_score: confidence_score,
            status: status.clone(),
        };

        (
            crate::domain::ConfidenceReport {
                confidence,
                confidence_score,
                reasons,
                status,
                ambiguity_score: Some(ambiguity_score),
            },
            breakdown,
        )
    }

    /// Convenience wrapper for callers that only need the ConfidenceReport.
    pub fn calculate_confidence(
        &self,
        query: &str,
        chunks: &[RetrievedChunk],
        strategy: &RetrievalStrategy,
        complexity: &crate::domain::QueryComplexity,
    ) -> crate::domain::ConfidenceReport {
        self.calculate_confidence_full(query, chunks, strategy, complexity).0
    }

    pub async fn retrieve_documents(
        &self,
        database: &Database,
        query: &str,
    ) -> Result<RetrievalResponse> {
        let mut analysis = self.query_analyzer_service.analyze(database, query).await?;
        
        // Exact Phrase Priority override and clear metadata filters for config queries
        if is_configuration_query(query) {
            analysis.strategy = RetrievalStrategy::Sparse;
            analysis.metadata_filters = MetadataFilters::default();
        }

        let results = self
            .retrieve_with_strategy(database, query, &analysis.strategy, &analysis, 0)
            .await?;

        Ok(RetrievalResponse {
            query: query.to_string(),
            strategy_used: analysis.strategy.clone(),
            total_results: results.len(),
            analysis,
            results,
            confidence: None,
        })
    }

    pub async fn retrieve_with_strategy(
        &self,
        database: &Database,
        query: &str,
        strategy: &RetrievalStrategy,
        analysis: &QueryAnalysis,
        depth: usize,
    ) -> Result<Vec<RetrievedChunk>> {
        let search_query = expand_query(query);
        match strategy {
            RetrievalStrategy::Dense => {
                self.retrieve_dense(database, &search_query, analysis.metadata_filters.as_ref(), 20)
                    .await
            }
            RetrievalStrategy::Sparse => {
                self.retrieve_sparse(database, &search_query, analysis.metadata_filters.as_ref(), 20)
                    .await
            }
            RetrievalStrategy::Hybrid => self.retrieve_hybrid(database, &search_query, analysis, 30).await,
            RetrievalStrategy::Faceted => self.retrieve_faceted(database, &search_query, analysis).await,
            RetrievalStrategy::Contextual => self.retrieve_contextual(database, &search_query, analysis).await,
            RetrievalStrategy::Recursive => self.retrieve_recursive(database, &search_query, analysis, depth).await,
        }
    }

    async fn retrieve_dense(
        &self,
        database: &Database,
        query: &str,
        filters: Option<&MetadataFilters>,
        limit: usize,
    ) -> Result<Vec<RetrievedChunk>> {
        let query_embeddings = self.ollama_service.generate_embeddings(&[query.to_string()]).await?;
        let Some(query_vector) = query_embeddings.into_iter().next() else {
            return Err(anyhow!("query embedding generation returned no vectors"));
        };

        let filter = filters.and_then(build_qdrant_filter);
        let results = self
            .qdrant_service
            .search_similar_points(query_vector, limit, filter)
            .await?;

        self.hydrate_qdrant_results(database, results)
    }

    async fn retrieve_sparse(
        &self,
        database: &Database,
        query: &str,
        filters: Option<&MetadataFilters>,
        limit: usize,
    ) -> Result<Vec<RetrievedChunk>> {
        let sparse_hits = self
            .sparse_service
            .search(query, filters, limit)
            .await?;
        let ordered_chunk_ids = sparse_hits
            .iter()
            .map(|hit| hit.chunk_id.clone())
            .collect::<Vec<_>>();
        let mut hydrated = hydrate_chunk_ids(database, &ordered_chunk_ids)?;
        let score_map = sparse_hits
            .into_iter()
            .map(|hit| (hit.chunk_id, hit.score))
            .collect::<HashMap<_, _>>();

        for chunk in &mut hydrated {
            chunk.score = score_map.get(&chunk.chunk_id).copied().unwrap_or_default();
        }

        sort_descending(&mut hydrated);
        Ok(hydrated)
    }

    async fn retrieve_hybrid(
        &self,
        database: &Database,
        query: &str,
        analysis: &QueryAnalysis,
        limit: usize,
    ) -> Result<Vec<RetrievedChunk>> {
        let dense = self
            .retrieve_dense(database, query, analysis.metadata_filters.as_ref(), 20)
            .await?;
        let sparse = self
            .retrieve_sparse(database, query, analysis.metadata_filters.as_ref(), 20)
            .await?;

        let mut scores = HashMap::<String, f32>::new();
        let mut chunk_map = HashMap::<String, RetrievedChunk>::new();

        for (rank, chunk) in dense.into_iter().enumerate() {
            let score = reciprocal_rank_fusion(rank + 1);
            *scores.entry(chunk.chunk_id.clone()).or_default() += score;
            chunk_map.entry(chunk.chunk_id.clone()).or_insert(chunk);
        }

        for (rank, chunk) in sparse.into_iter().enumerate() {
            let score = reciprocal_rank_fusion(rank + 1);
            *scores.entry(chunk.chunk_id.clone()).or_default() += score;
            chunk_map.entry(chunk.chunk_id.clone()).or_insert(chunk);
        }

        let mut merged = chunk_map
            .into_iter()
            .map(|(chunk_id, mut chunk)| {
                chunk.score = scores.get(&chunk_id).copied().unwrap_or_default();
                chunk
            })
            .collect::<Vec<_>>();

        sort_descending(&mut merged);
        merged.truncate(limit);
        Ok(merged)
    }

    async fn retrieve_faceted(
        &self,
        database: &Database,
        query: &str,
        analysis: &QueryAnalysis,
    ) -> Result<Vec<RetrievedChunk>> {
        let mut results = self.retrieve_hybrid(database, query, analysis, 30).await?;
        results.retain(|chunk| matches_filters(chunk, &analysis.metadata_filters));
        Ok(results)
    }

    async fn retrieve_contextual(
        &self,
        database: &Database,
        query: &str,
        analysis: &QueryAnalysis,
    ) -> Result<Vec<RetrievedChunk>> {
        let mut results = self.retrieve_hybrid(database, query, analysis, 30).await?;
        let now = Utc::now();

        for chunk in &mut results {
            if let Some(modified_at) = chunk.modified_at.as_deref() {
                if let Ok(parsed) = DateTime::parse_from_rfc3339(modified_at) {
                    let days = now
                        .signed_duration_since(parsed.with_timezone(&Utc))
                        .num_days()
                        .max(0) as i32;
                    chunk.score *= 0.95_f32.powi(days);
                }
            }

            if is_upcoming_event(&chunk.metadata) {
                chunk.score *= 2.0;
            }
        }

        sort_descending(&mut results);
        Ok(results)
    }

    async fn retrieve_recursive(
        &self,
        database: &Database,
        query: &str,
        analysis: &QueryAnalysis,
        depth: usize,
    ) -> Result<Vec<RetrievedChunk>> {
        // Step 1: Hop 1 Retrieval
        // Clear all metadata filters for recursive multi-hop retrieval to allow cross-source and cross-tag queries
        let mut clean_analysis = analysis.clone();
        clean_analysis.metadata_filters = MetadataFilters::default();

        // Deterministic query splitting to handle multi-topic/multi-hop search queries
        let mut sub_queries = Vec::new();
        let lower_query = query.to_lowercase();
        
        if let Some(pos) = lower_query.find(" connect to ") {
            let left = query[..pos].trim().to_string();
            let right = query[pos + " connect to ".len()..].trim().to_string();
            sub_queries.push(left);
            sub_queries.push(right);
        } else if let Some(pos) = lower_query.find(" connect ") {
            let left = query[..pos].trim().to_string();
            let right = query[pos + " connect ".len()..].trim().to_string();
            sub_queries.push(left);
            sub_queries.push(right);
        } else if let Some(pos) = lower_query.find(" relate to ") {
            let left = query[..pos].trim().to_string();
            let right = query[pos + " relate to ".len()..].trim().to_string();
            sub_queries.push(left);
            sub_queries.push(right);
        } else if let Some(pos) = lower_query.find(" link to ") {
            let left = query[..pos].trim().to_string();
            let right = query[pos + " link to ".len()..].trim().to_string();
            sub_queries.push(left);
            sub_queries.push(right);
        } else {
            sub_queries.push(query.to_string());
        }

        // [RECURSIVE] Hop 1 instrumentation
        let mut hop1_chunks = Vec::new();
        let sub_queries_cloned = sub_queries.clone();
        tracing::info!(
            "[RECURSIVE hop=1] query={:?} sub_queries={:?}",
            query, sub_queries
        );
        for sub_q in sub_queries {
            let search_query = expand_query(&sub_q);
            let contains_exact_phrase = is_configuration_query(&sub_q);
            let sub_chunks = if contains_exact_phrase {
                self.retrieve_sparse(database, &search_query, clean_analysis.metadata_filters.as_ref(), 12).await?
            } else {
                self.retrieve_hybrid(database, &search_query, &clean_analysis, 12).await?
            };
            tracing::info!(
                "[RECURSIVE hop=1] sub_query={:?} retrieved={} titles={:?}",
                sub_q, sub_chunks.len(),
                sub_chunks.iter().take(5).map(|c| c.document_title.as_str()).collect::<Vec<_>>()
            );
            hop1_chunks.extend(sub_chunks);
        }
        deduplicate_chunks(&mut hop1_chunks);

        // Attach Hop 1 Lineage
        for chunk in &mut hop1_chunks {
            let mut meta = chunk.metadata.as_object().cloned().unwrap_or_default();
            meta.insert("lineage".to_string(), json!({
                "hop_number": 1,
                "parent_chunk": serde_json::Value::Null,
                "retrieval_reason": "Initial query retrieval"
            }));
            chunk.metadata = Value::Object(meta);
        }

        // Limit the depth and hops to maximum 2 hops (depth 0 is Hop 1, depth 1 is Hop 2)
        if depth >= 1 {
            return Ok(hop1_chunks);
        }

        // Step 2: Deterministic Reference Extraction from Hop 1 content
        let all_docs = database.document_repository().list_all_chunk_search_documents()?;
        
        // Build variants for title matching (underscores, spaces, dashes)
        let mut doc_title_variants = Vec::new();
        for doc in all_docs {
            let title_lower = doc.title.to_lowercase();
            doc_title_variants.push((doc.document_id.clone(), title_lower.clone(), doc.title.clone()));
            
            let with_spaces = title_lower.replace('_', " ");
            if with_spaces != title_lower {
                doc_title_variants.push((doc.document_id.clone(), with_spaces, doc.title.clone()));
            }
            
            let with_dashes = title_lower.replace('_', "-");
            if with_dashes != title_lower {
                doc_title_variants.push((doc.document_id.clone(), with_dashes, doc.title.clone()));
            }
        }

        let mut referenced_docs = Vec::new(); // Tuple of (document_id, parent_chunk_id, parent_doc_title)
        let wikilink_re = regex::Regex::new(r"\[\[([a-zA-Z0-9_\-\s:]+)\]\]").unwrap();
        let md_link_re = regex::Regex::new(r"\[[^\]]+\]\(\.\./[^/]+/([a-zA-Z0-9_\-]+)\.md\)").unwrap();

        for chunk in &hop1_chunks {
            let content_lower = chunk.content.to_lowercase();
            
            // Extract from wikilinks
            for cap in wikilink_re.captures_iter(&chunk.content) {
                let ref_title = cap[1].trim().to_lowercase();
                for (doc_id, variant, original_title) in &doc_title_variants {
                    if ref_title == *variant && *doc_id != chunk.document_id {
                        referenced_docs.push((doc_id.clone(), chunk.chunk_id.clone(), original_title.clone()));
                    }
                }
            }

            // Extract from markdown links
            for cap in md_link_re.captures_iter(&chunk.content) {
                let ref_name = cap[1].trim().to_lowercase();
                for (doc_id, variant, original_title) in &doc_title_variants {
                    if ref_name == *variant && *doc_id != chunk.document_id {
                        referenced_docs.push((doc_id.clone(), chunk.chunk_id.clone(), original_title.clone()));
                    }
                }
            }

            // Also check for direct document title matches in content
            for (doc_id, variant, original_title) in &doc_title_variants {
                if variant.len() < 5 {
                    continue;
                }
                if content_lower.contains(variant) && *doc_id != chunk.document_id {
                    referenced_docs.push((doc_id.clone(), chunk.chunk_id.clone(), original_title.clone()));
                }
            }
        }

        referenced_docs.dedup_by(|a, b| a.0 == b.0);

        // [RECURSIVE] Hop 2 instrumentation
        let hop2_found_via_links = referenced_docs.len();
        tracing::info!(
            "[RECURSIVE hop=2] query={:?} wikilink_keyword_refs_found={} (before dedup)",
            query, hop2_found_via_links
        );

        // Step 3: Hop 2 Retrieval
        let mut hop2_chunks = Vec::new();
        for (doc_id, parent_chunk_id, parent_doc_title) in referenced_docs.into_iter().take(5) {
            let chunks_in_doc = database.document_repository().get_chunks_by_document(&doc_id)?;
            let chunk_ids: Vec<String> = chunks_in_doc.into_iter().map(|c| c.id).collect();
            let mut hydrated_hop2 = hydrate_chunk_ids(database, &chunk_ids)?;
            tracing::info!(
                "[RECURSIVE hop=2] cross_ref_doc={:?} fetched_chunks={}",
                parent_doc_title, hydrated_hop2.len()
            );
            
            // Attach Hop 2 Lineage
            for chunk in &mut hydrated_hop2 {
                let mut meta = chunk.metadata.as_object().cloned().unwrap_or_default();
                meta.insert("lineage".to_string(), json!({
                    "hop_number": 2,
                    "parent_chunk": parent_chunk_id.clone(),
                    "retrieval_reason": format!("Cross-reference: {}", parent_doc_title)
                }));
                chunk.metadata = Value::Object(meta);
            }
            hop2_chunks.extend(hydrated_hop2);
        }

        // ── Hop 2 Semantic Keyword Fallback ─────────────────────────────────────────
        // If wikilink/content-keyword extraction yielded no referenced docs AND we
        // had multiple sub-queries (multi-hop intent), directly run Hybrid retrieval
        // on each sub-query beyond the first. This covers the case where Hop 1 docs
        // don't contain explicit links to the target (e.g. onboarding doc has no
        // [[notion_setup]] wikilink, but the user asked about onboarding + Notion).
        if hop2_chunks.is_empty() && sub_queries_cloned.len() >= 2 {
            tracing::info!(
                "[RECURSIVE hop=2] NO_WIKILINKS_FOUND — falling back to direct hybrid retrieval on remaining sub-queries"
            );
            for sub_q in sub_queries_cloned.iter().skip(1) {
                let search_query = expand_query(sub_q);
                let mut fallback_chunks = self
                    .retrieve_hybrid(database, &search_query, &clean_analysis, 10)
                    .await?;
                tracing::info!(
                    "[RECURSIVE hop=2] fallback sub_query={:?} retrieved={} titles={:?}",
                    sub_q, fallback_chunks.len(),
                    fallback_chunks.iter().take(5).map(|c| c.document_title.as_str()).collect::<Vec<_>>()
                );
                // Mark as Hop 2 with fallback reason
                for chunk in &mut fallback_chunks {
                    let mut meta = chunk.metadata.as_object().cloned().unwrap_or_default();
                    meta.insert("lineage".to_string(), json!({
                        "hop_number": 2,
                        "parent_chunk": null,
                        "retrieval_reason": format!("Semantic fallback: {}", sub_q)
                    }));
                    chunk.metadata = Value::Object(meta);
                }
                hop2_chunks.extend(fallback_chunks);
            }
        }

        tracing::info!(
            "[RECURSIVE] query={:?} hop1_count={} hop2_count={} total_before_dedup={}",
            query,
            hop2_chunks.len() + (if hop2_chunks.is_empty() { 0 } else { 0 }), // logged separately above
            hop2_chunks.len(),
            hop1_chunks.len() + hop2_chunks.len()
        );

        // Step 4: Merge results
        let mut accumulated = hop1_chunks;
        accumulated.extend(hop2_chunks);
        deduplicate_chunks(&mut accumulated);

        // Step 5: Globally Rerank
        sort_descending(&mut accumulated);
        let reranked = self.reranker_service.rerank(query, accumulated).await?;
        Ok(reranked)
    }

    async fn generate_answer(
        &self,
        query: &str,
        analysis: &QueryAnalysis,
        context: &BuiltContext,
    ) -> Result<String> {
        let system_prompt = "You are Assistant Core. Answer using only the supplied grounded context. If the context is insufficient, say so clearly. Do not invent citations. Keep the answer concise but helpful.";
        let user_prompt = format!(
            "User query: {query}\nStrategy: {:?}\nGrounded context:\n{}\nProvide the final answer only.",
            analysis.strategy,
            context.context_text
        );
        self.groq_service.chat_text(system_prompt, &user_prompt).await
    }

    pub async fn initialize(&self, database: &Database) -> Result<()> {
        self.ensure_groq_ready()?;
        self.sparse_service.initialize().await?;
        let documents = database.document_repository().list_all_chunk_search_documents()?;
        self.sparse_service.rebuild_index(&documents).await?;
        self.reranker_service.initialize().await?;
        Ok(())
    }

    pub async fn clear_sparse_index(&self) -> Result<()> {
        self.sparse_service.clear_index().await
    }

    fn hydrate_qdrant_results(
        &self,
        database: &Database,
        results: Vec<QdrantSearchResult>,
    ) -> Result<Vec<RetrievedChunk>> {
        let ordered_chunk_ids = results.iter().map(|result| result.id.clone()).collect::<Vec<_>>();
        let mut hydrated = hydrate_chunk_ids(database, &ordered_chunk_ids)?;
        let score_map = results
            .into_iter()
            .map(|result| (result.id, result.score))
            .collect::<HashMap<_, _>>();

        for chunk in &mut hydrated {
            chunk.score = score_map.get(&chunk.chunk_id).copied().unwrap_or_default();
        }
        sort_descending(&mut hydrated);
        Ok(hydrated)
    }
}

impl RetrievalService {
    fn ensure_groq_ready(&self) -> Result<()> {
        if self.groq_service.is_configured() {
            Ok(())
        } else {
            Err(anyhow!("GROQ_API_KEY is not configured"))
        }
    }
}

fn hydrate_chunk_ids(database: &Database, ordered_chunk_ids: &[String]) -> Result<Vec<RetrievedChunk>> {
    let rows = database
        .document_repository()
        .get_chunk_search_documents_by_ids(ordered_chunk_ids)?;
    let row_map = rows
        .into_iter()
        .map(|row| (row.chunk_id.clone(), row))
        .collect::<HashMap<_, _>>();

    let mut hydrated = Vec::new();
    for chunk_id in ordered_chunk_ids {
        if let Some(row) = row_map.get(chunk_id) {
            let mut merged_meta = row.metadata.clone();
            if let Some(obj) = merged_meta.as_object_mut() {
                if let Some(chunk_meta_str) = &row.chunk_metadata_json {
                    if let Ok(Value::Object(chunk_obj)) = serde_json::from_str::<Value>(chunk_meta_str) {
                        for (k, v) in chunk_obj {
                            obj.insert(k, v);
                        }
                    }
                }
            }
            hydrated.push(RetrievedChunk {
                chunk_id: row.chunk_id.clone(),
                document_id: row.document_id.clone(),
                source: row.source_kind.clone(),
                document_title: row.title.clone(),
                content: row.content.clone(),
                score: 0.0,
                retrieval_score: None,
                ordinal: row.ordinal,
                path_or_url: row.path_or_url.clone(),
                tags: row.tags.clone(),
                author: row.author.clone(),
                category: row.category.clone(),
                created_at: row.created_at.clone(),
                modified_at: row.updated_at.clone(),
                metadata: merged_meta,
            });
        }
    }

    Ok(hydrated)
}

// Deterministic Helper Functions

fn expand_query(query: &str) -> String {
    let mut expanded = query.to_string();
    let lower = query.to_lowercase();
    if lower.contains("chunk size") {
        expanded.push_str(" chunking size token window chunk configuration");
    }
    if lower.contains("embedding model") {
        expanded.push_str(" embeddings vector model");
    }
    // Authentication / credential expansion
    if lower.contains("authentication") || lower.contains(" auth") {
        expanded.push_str(" login credential token jwt oauth session access");
    }
    // Onboarding expansion
    if lower.contains("onboarding") {
        expanded.push_str(" setup guide welcome new user workflow orientation");
    }
    // Monitoring expansion
    if lower.contains("monitoring") || lower.contains("monitor") {
        expanded.push_str(" metrics logs alerts observability telemetry health");
    }
    // Credential / storage expansion
    if lower.contains("credential") {
        expanded.push_str(" password keychain secure storage encrypted token");
    }
    expanded
}

fn is_configuration_query(query: &str) -> bool {
    let lower = query.to_lowercase();
    lower.contains("chunk size")
        || lower.contains("overlap")
        || lower.contains("embedding model")
        || lower.contains("dimension")
        || lower.contains("threshold")
        || lower.contains("timeout")
        || lower.contains("token limit")
        || lower.contains("parameter")
        || lower.contains("setting")
        || lower.contains("configuration")
        || lower.contains("constant")
        || lower.contains("limit")
}

fn boost_configuration_chunks(chunks: &mut [RetrievedChunk]) {
    for chunk in chunks {
        let lower_content = chunk.content.to_lowercase();
        
        let has_config_term = lower_content.contains("chunk size")
            || lower_content.contains("overlap")
            || lower_content.contains("embedding model")
            || lower_content.contains("dimension")
            || lower_content.contains("threshold")
            || lower_content.contains("timeout")
            || lower_content.contains("token limit");

        let has_setting_pattern = lower_content.contains('=') 
            || lower_content.contains("tokens") 
            || lower_content.contains("overlap") 
            || lower_content.contains("dimension")
            || lower_content.contains("factor")
            || lower_content.contains("percent");

        if has_config_term || has_setting_pattern {
            chunk.score *= 1.35;
        }
    }
}

#[derive(Debug, Clone)]
struct DocumentAggregation {
    document_id: String,
    document_title: String,
    source: String,
    document_score: f32,
    chunk_count: usize,
}

fn aggregate_documents(chunks: &[RetrievedChunk]) -> Vec<DocumentAggregation> {
    let mut groups: HashMap<String, (String, String, Vec<f32>)> = HashMap::new();
    for chunk in chunks {
        let entry = groups.entry(chunk.document_id.clone())
            .or_insert_with(|| (chunk.document_title.clone(), chunk.source.clone(), Vec::new()));
        entry.2.push(chunk.score);
    }

    let mut docs: Vec<DocumentAggregation> = groups.into_iter().map(|(doc_id, (title, source, scores))| {
        let chunk_count = scores.len();
        let sum: f32 = scores.iter().sum();
        let document_score = if chunk_count > 0 { sum / chunk_count as f32 } else { 0.0 };
        DocumentAggregation {
            document_id: doc_id,
            document_title: title,
            source,
            document_score,
            chunk_count,
        }
    }).collect();

    docs.sort_by(|a, b| b.document_score.partial_cmp(&a.document_score).unwrap_or(std::cmp::Ordering::Equal));
    docs
}

fn clean_document_title(title: &str) -> String {
    let mut cleaned = title.replace('_', " ");
    cleaned = cleaned.split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    cleaned
}

fn reciprocal_rank_fusion(rank: usize) -> f32 {
    1.0 / (60.0 + rank as f32)
}

fn sort_descending(chunks: &mut [RetrievedChunk]) {
    chunks.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

fn deduplicate_chunks(chunks: &mut Vec<RetrievedChunk>) {
    let mut seen = HashSet::new();
    chunks.retain(|chunk| seen.insert(chunk.chunk_id.clone()));
}

fn build_qdrant_filter(filters: &MetadataFilters) -> Option<QdrantSearchFilter> {
    let mut must = Vec::new();

    if let Some(sources) = &filters.source {
        must.push(json!({
            "key": "source",
            "match": { "any": sources }
        }));
    }

    if let Some(authors) = &filters.author {
        let lowered: Vec<String> = authors.iter().map(|s| s.to_lowercase()).collect();
        must.push(json!({
            "key": "author",
            "match": { "any": lowered }
        }));
    }

    if let Some(tags) = &filters.tags {
        let lowered: Vec<String> = tags.iter().map(|s| s.to_lowercase()).collect();
        must.push(json!({
            "key": "tags",
            "match": { "any": lowered }
        }));
    }

    if let Some(categories) = &filters.category {
        let lowered: Vec<String> = categories.iter().map(|s| s.to_lowercase()).collect();
        must.push(json!({
            "key": "category",
            "match": { "any": lowered }
        }));
    }

    if let Some(date_range) = &filters.date_range {
        if date_range.from.is_some() || date_range.to.is_some() {
            let mut range = serde_json::Map::new();
            if let Some(from) = &date_range.from {
                range.insert("gte".to_string(), Value::String(from.clone()));
            }
            if let Some(to) = &date_range.to {
                range.insert("lte".to_string(), Value::String(to.clone()));
            }
            must.push(json!({
                "key": "modified_at",
                "range": Value::Object(range),
            }));
        }
    }

    if must.is_empty() {
        None
    } else {
        Some(QdrantSearchFilter {
            must,
            should: Vec::new(),
            must_not: Vec::new(),
        })
    }
}

fn matches_filters(chunk: &RetrievedChunk, filters: &MetadataFilters) -> bool {
    if let Some(sources) = &filters.source {
        if !sources
            .iter()
            .any(|source| source.eq_ignore_ascii_case(&chunk.source))
        {
            return false;
        }
    }

    if let Some(authors) = &filters.author {
        let Some(author) = &chunk.author else {
            return false;
        };
        if !authors
            .iter()
            .any(|candidate| author.to_lowercase().contains(&candidate.to_lowercase()))
        {
            return false;
        }
    }

    if let Some(tags) = &filters.tags {
        if !tags.iter().any(|required| {
            chunk
                .tags
                .iter()
                .any(|tag| tag.to_lowercase().contains(&required.to_lowercase()))
        }) {
            return false;
        }
    }

    if let Some(categories) = &filters.category {
        let Some(category) = &chunk.category else {
            return false;
        };
        if !categories.iter().any(|value| {
            category
                .to_lowercase()
                .contains(&value.to_lowercase())
        }) {
            return false;
        }
    }

    if let Some(date_range) = &filters.date_range {
        let date = chunk
            .modified_at
            .as_deref()
            .or(chunk.created_at.as_deref())
            .unwrap_or_default();
        if let Some(from) = &date_range.from {
            if date < from.as_str() {
                return false;
            }
        }
        if let Some(to) = &date_range.to {
            if date > to.as_str() {
                return false;
            }
        }
    }

    true
}

fn is_upcoming_event(metadata: &Value) -> bool {
    metadata
        .get("is_upcoming_event")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| {
            metadata
                .get("upcoming")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
}

fn is_ambiguous_retrieval(
    query: &str,
    chunks: &[RetrievedChunk],
    strategy: &RetrievalStrategy,
) -> bool {
    // Recursive strategy fetches from multiple docs intentionally — never ambiguous.
    if strategy == &RetrievalStrategy::Recursive {
        return false;
    }
    if chunks.len() < 2 {
        return false;
    }

    // ── GATE 1: Factual Lookup Guard ────────────────────────────────────────────
    // Factual lookup queries ("what", "which", "where", "when", "how many",
    // "does", "is") have a single correct answer. Multiple docs containing
    // that answer are EVIDENCE REINFORCEMENT, not ambiguity. Never fire.
    let lower_query = query.to_lowercase();
    let factual_prefixes = [
        "what ", "what's ", "which ", "where ", "when ", "how many ",
        "how much ", "does ", "is the ", "is there ", "are there ",
    ];
    if factual_prefixes.iter().any(|p| lower_query.starts_with(p)) {
        return false;
    }

    // Build the unique document set from the top-5 chunks.
    let mut unique_doc_ids: Vec<String> = Vec::new();
    for chunk in chunks.iter().take(5) {
        if !unique_doc_ids.contains(&chunk.document_id) {
            unique_doc_ids.push(chunk.document_id.clone());
        }
    }
    // If all top chunks are from the same document, trivially not ambiguous.
    if unique_doc_ids.len() < 2 {
        return false;
    }

    let stop_words: HashSet<&str> = [
        "what", "is", "the", "does", "how", "to", "explain", "describe", "about",
        "system", "use", "connect", "setup", "integration", "with", "and", "or",
        "a", "an", "of", "which", "whose", "where", "whom", "related", "that",
        "this", "for", "from", "are", "was", "were", "has", "have", "had",
        "can", "will", "would", "should", "could", "may", "might", "in", "on",
    ].iter().cloned().collect();

    let tokenize = |text: &str| -> HashSet<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && !stop_words.contains(s.as_str()) && s.len() > 3)
            .collect()
    };

    // ── GATE 2: Any Shared Title Keyword Guard ───────────────────────────────────
    // If ANY meaningful keyword (len > 4) appears in ALL top-doc titles, they are
    // in the same domain. This catches cases like rag_hybrid, rag_recursive, rag_arch
    // all sharing the prefix "rag", or auth_sso, auth_token, auth_flow sharing "auth".
    let title_token_sets: Vec<HashSet<String>> = unique_doc_ids.iter()
        .map(|doc_id| {
            chunks.iter().find(|c| &c.document_id == doc_id)
                .map(|c| tokenize(&c.document_title))
                .unwrap_or_default()
        })
        .collect();

    if title_token_sets.len() >= 2 && !title_token_sets[0].is_empty() {
        // Check if any token from the first title appears in ALL other titles
        let shared_by_all = title_token_sets[0].iter().any(|token| {
            token.len() > 4 && title_token_sets.iter().skip(1).all(|other| other.contains(token))
        });
        if shared_by_all {
            return false;
        }
    }

    // ── GATE 3: Full Topic Divergence Check (tightened thresholds) ──────────────
    // Collect keyword set per document (title + up to 400 chars of chunk content)
    // for a more complete topic signal than the previous 200-char window.
    let mut doc_token_sets: Vec<(String, HashSet<String>)> = Vec::new();
    for doc_id in &unique_doc_ids {
        let mut tokens = HashSet::new();
        for chunk in chunks.iter().take(5).filter(|c| &c.document_id == doc_id) {
            tokens.extend(tokenize(&chunk.document_title));
            let content_snippet: String = chunk.content.chars().take(400).collect();
            tokens.extend(tokenize(&content_snippet));
        }
        doc_token_sets.push((doc_id.clone(), tokens));
    }

    // Check if ANY document pair has high enough topic overlap to be same-domain.
    // Using Jaccard < 0.15 (down from 0.40) to allow more variation within a topic.
    let (_, top_tokens) = &doc_token_sets[0];
    let mut all_divergent = true;
    for (_, other_tokens) in doc_token_sets.iter().skip(1) {
        if top_tokens.is_empty() || other_tokens.is_empty() {
            all_divergent = false;
            break;
        }
        let intersection = top_tokens.intersection(other_tokens).count();
        let union = top_tokens.union(other_tokens).count();
        let jaccard = intersection as f32 / union as f32;
        if jaccard >= 0.15 {
            // At least one doc-pair is in the same topic domain.
            all_divergent = false;
            break;
        }
    }

    if !all_divergent {
        return false;
    }

    // Documents ARE in genuinely different topic domains. Only trigger AMBIGUOUS
    // if the reranker cannot confidently distinguish which the user wanted
    // (score gap < 0.06, tightened from 0.12).
    let doc_aggregations = aggregate_documents(chunks);
    if doc_aggregations.len() < 2 {
        return false;
    }
    let sigmoid = |x: f32| -> f32 { 1.0 / (1.0 + (-x).exp()) };
    let top_score = sigmoid(doc_aggregations[0].document_score);
    let second_score = sigmoid(doc_aggregations[1].document_score);
    let score_gap = top_score - second_score;

    // Require BOTH: genuinely different topics AND very close scores (< 0.06 gap).
    // This prevents false ambiguity when one document is clearly more relevant.
    score_gap < 0.06
}

trait MetadataFiltersExt {
    fn as_ref(&self) -> Option<&MetadataFilters>;
}

impl MetadataFiltersExt for MetadataFilters {
    fn as_ref(&self) -> Option<&MetadataFilters> {
        Some(self)
    }
}

// ---------------------------------------------------------------------------
// Keyword Overlap Helper
// ---------------------------------------------------------------------------

/// Computes Jaccard similarity between the query token set and the combined
/// token set of the top-3 retrieved chunks. Used as a confidence signal.
fn keyword_overlap_score(query: &str, chunks: &[RetrievedChunk]) -> f32 {
    let stop_words: HashSet<&str> = [
        "what", "is", "the", "does", "how", "to", "explain", "describe", "about",
        "system", "use", "connect", "setup", "with", "and", "or", "a", "an", "of",
        "which", "where", "that", "this", "for", "from", "are", "was", "were",
        "has", "have", "had", "can", "will", "would", "should", "could", "in", "on",
    ].iter().cloned().collect();

    let tokenize = |text: &str| -> HashSet<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && !stop_words.contains(s.as_str()) && s.len() > 3)
            .collect()
    };

    let query_tokens = tokenize(query);
    if query_tokens.is_empty() {
        return 0.0;
    }

    let mut chunk_tokens: HashSet<String> = HashSet::new();
    for chunk in chunks.iter().take(3) {
        chunk_tokens.extend(tokenize(&chunk.content));
        chunk_tokens.extend(tokenize(&chunk.document_title));
    }

    if chunk_tokens.is_empty() {
        return 0.0;
    }

    let intersection = query_tokens.intersection(&chunk_tokens).count();
    let union = query_tokens.union(&chunk_tokens).count();
    intersection as f32 / union as f32
}

// ---------------------------------------------------------------------------
// Title-Keyword Bonus
// ---------------------------------------------------------------------------

/// Returns +15 if any meaningful keyword from the query (len > 4, not a stop word)
/// appears verbatim (case-insensitive) in the top retrieved chunk's document title.
///
/// Rationale: if the user asks "Explain authentication" and the top doc is titled
/// "authentication_flow_oauth2", the noun "authentication" IS in the title. That is a
/// very strong direct-match signal that should boost confidence regardless of logit value.
fn query_title_match_bonus(query: &str, chunks: &[RetrievedChunk]) -> i32 {
    let Some(top_chunk) = chunks.first() else { return 0 };
    let title_lower = top_chunk.document_title.to_lowercase();

    let stop_words: HashSet<&str> = [
        "what", "is", "the", "does", "how", "to", "explain", "describe", "about",
        "system", "use", "connect", "setup", "with", "and", "or", "a", "an", "of",
        "which", "where", "that", "this", "for", "from", "are", "was", "were",
        "tell", "give", "show", "find", "get",
    ].iter().cloned().collect();

    for word in query.to_lowercase().split(|c: char| !c.is_alphanumeric()) {
        let w = word.trim();
        if w.len() > 4 && !stop_words.contains(w) && title_lower.contains(w) {
            return 15;
        }
    }
    0
}

// ---------------------------------------------------------------------------
// Fact-Level Citation Verification
// ---------------------------------------------------------------------------

/// Extracts factual anchors from a generated answer:
/// 1. Number + unit patterns (e.g. "512 tokens", "95%", "3 seconds")
/// 2. Capitalised proper nouns (e.g. "Qdrant", "Notion", "JWT", "OAuth")
/// 3. Backtick-quoted technical terms
fn extract_factual_anchors(answer: &str) -> Vec<String> {
    let mut anchors: Vec<String> = Vec::new();

    // 1. Number + unit (regex-free, scan for digit sequences followed by a unit word)
    let unit_words = ["token", "tokens", "ms", "second", "seconds", "percent",
                      "gb", "mb", "kb", "dimension", "dimensions", "chunk", "chunks",
                      "hop", "hops", "layer", "layers", "step", "steps"];
    let words: Vec<&str> = answer.split_whitespace().collect();
    for (i, word) in words.iter().enumerate() {
        let cleaned = word.trim_matches(|c: char| !c.is_alphanumeric());
        if cleaned.chars().all(|c| c.is_ascii_digit()) && !cleaned.is_empty() {
            // Peek at next word for unit
            if let Some(next) = words.get(i + 1) {
                let next_lower = next.to_lowercase();
                let next_clean = next_lower.trim_matches(|c: char| !c.is_alphanumeric());
                if unit_words.iter().any(|u| next_clean.starts_with(u)) {
                    anchors.push(format!("{} {}", cleaned, next_clean));
                    continue;
                }
            }
            anchors.push(cleaned.to_string());
        }

        // 2. Capitalised proper nouns (len > 3, not sentence-start, not all-caps acronyms ok)
        let first_char = cleaned.chars().next().unwrap_or('a');
        if first_char.is_uppercase() && cleaned.len() > 3 && i > 0 {
            // Exclude words that are just the first word of a sentence (previous ends with '.')
            let prev_ends_sentence = i > 0 && words[i - 1].ends_with('.');
            if !prev_ends_sentence {
                anchors.push(cleaned.to_string());
            }
        }
    }

    // 3. Backtick-quoted technical terms
    let mut in_backtick = false;
    let mut current = String::new();
    for ch in answer.chars() {
        if ch == '`' {
            if in_backtick && !current.is_empty() {
                anchors.push(current.clone());
                current.clear();
            }
            in_backtick = !in_backtick;
        } else if in_backtick {
            current.push(ch);
        }
    }

    // Deduplicate, ignore empty
    anchors.sort();
    anchors.dedup();
    anchors.retain(|a| !a.is_empty() && a.len() > 2);
    anchors
}

/// Returns the fraction of factual anchors found in the answer that can be
/// traced verbatim (case-insensitive) to at least one retrieved chunk.
/// Returns 1.0 if no anchors exist (nothing to verify).
fn verify_answer_against_chunks(answer: &str, chunks: &[RetrievedChunk]) -> f32 {
    let anchors = extract_factual_anchors(answer);
    if anchors.is_empty() {
        return 1.0;
    }
    let all_content = chunks.iter()
        .map(|c| c.content.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");

    let verified = anchors.iter()
        .filter(|anchor| all_content.contains(&anchor.to_lowercase()))
        .count();

    verified as f32 / anchors.len() as f32
}

// ---------------------------------------------------------------------------
// Recall Metrics
// ---------------------------------------------------------------------------

/// Computes recall metrics by comparing pre-rerank and post-rerank chunk lists.
/// `fact_coverage` is the citation verification score (0.0–1.0) from
/// `verify_answer_against_chunks`.
fn compute_recall_metrics(
    pre_rerank: &[RetrievedChunk],
    post_rerank: &[RetrievedChunk],
    fact_coverage: f32,
) -> crate::domain::RecallMetrics {
    let pre_titles: Vec<String> = pre_rerank.iter()
        .take(20)
        .map(|c| c.document_title.clone())
        .collect();
    let post_titles: Vec<String> = post_rerank.iter()
        .take(10)
        .map(|c| c.document_title.clone())
        .collect();

    let unique_pre: HashSet<&str> = pre_rerank.iter()
        .map(|c| c.document_id.as_str())
        .collect();
    let unique_post: HashSet<&str> = post_rerank.iter()
        .map(|c| c.document_id.as_str())
        .collect();

    let top_doc_changed = pre_rerank.first().map(|c| c.document_id.as_str())
        != post_rerank.first().map(|c| c.document_id.as_str());

    crate::domain::RecallMetrics {
        pre_rerank_doc_titles: pre_titles,
        post_rerank_doc_titles: post_titles,
        unique_docs_pre_rerank: unique_pre.len(),
        unique_docs_post_rerank: unique_post.len(),
        top_doc_changed,
        pre_rerank_top_score: pre_rerank.first().map(|c| c.score).unwrap_or(0.0),
        post_rerank_top_score: post_rerank.first().map(|c| c.score).unwrap_or(0.0),
        fact_coverage,
    }
}


mod tests {
    use super::*;

    #[test]
    fn reciprocal_rank_fusion_uses_exact_formula() {
        let score = reciprocal_rank_fusion(1);
        assert!((score - (1.0 / 61.0)).abs() < f32::EPSILON);
    }

    fn mock_chunk(doc_id: &str, score: f32, retrieval_score: f32) -> RetrievedChunk {
        RetrievedChunk {
            chunk_id: uuid::Uuid::new_v4().to_string(),
            document_id: doc_id.to_string(),
            source: "obsidian".to_string(),
            document_title: format!("Mock Title {}", doc_id),
            content: "Mock Content".to_string(),
            score,
            retrieval_score: Some(retrieval_score),
            ordinal: 0,
            path_or_url: None,
            tags: Vec::new(),
            author: None,
            category: None,
            created_at: None,
            modified_at: None,
            metadata: serde_json::Value::Null,
        }
    }

    fn mock_chunk_with_content(doc_id: &str, title: &str, content: &str, score: f32) -> RetrievedChunk {
        RetrievedChunk {
            chunk_id: uuid::Uuid::new_v4().to_string(),
            document_id: doc_id.to_string(),
            source: "obsidian".to_string(),
            document_title: title.to_string(),
            content: content.to_string(),
            score,
            retrieval_score: Some(score),
            ordinal: 0,
            path_or_url: None,
            tags: Vec::new(),
            author: None,
            category: None,
            created_at: None,
            modified_at: None,
            metadata: serde_json::Value::Null,
        }
    }

    fn mock_service() -> RetrievalService {
        RetrievalService::new(
            crate::services::ollama::OllamaService::new("http://localhost".to_string(), "emb".to_string()),
            crate::services::qdrant::QdrantService::new("http://localhost".to_string(), "coll".to_string()),
            crate::services::sparse::SparseRetrievalService::new(8743, std::path::PathBuf::from("/tmp/f"), "node".to_string()),
            crate::services::groq::GroqService::new(None, None, None, "http://localhost".to_string(), "m1".to_string(), "m2".to_string()),
            crate::services::query_analyzer::QueryAnalyzerService::new(
                crate::services::groq::GroqService::new(None, None, None, "http://localhost".to_string(), "m1".to_string(), "m2".to_string())
            ),
            crate::services::reranker::RerankerService::new(8742, std::path::PathBuf::from("."), std::path::PathBuf::from("."), "m".to_string(), None),
            crate::services::context_builder::ContextBuilder::new(),
        )
    }

    // ── Confidence Status Tests (updated for relaxed thresholds) ──────────────

    #[test]
    fn test_confidence_empty_retrieval() {
        let service = mock_service();
        // No chunks → EMPTY_RETRIEVAL
        let report = service.calculate_confidence(
            "test query",
            &[],
            &RetrievalStrategy::Hybrid,
            &crate::domain::QueryComplexity::Simple,
        );
        assert_eq!(report.status, "EMPTY_RETRIEVAL");
        assert_eq!(report.confidence, "low");
        assert_eq!(report.confidence_score, 0);

        // score=-3 → sigmoid(-3)=0.047 > new empty_threshold=0.05? No, 0.047 < 0.05 → EMPTY
        let weak_chunks = vec![mock_chunk("doc-1", -3.0, 0.001)];
        let report2 = service.calculate_confidence(
            "test query",
            &weak_chunks,
            &RetrievalStrategy::Hybrid,
            &crate::domain::QueryComplexity::Simple,
        );
        assert_eq!(report2.status, "EMPTY_RETRIEVAL");
    }

    #[test]
    fn test_confidence_low_confidence_retrieval() {
        let service = mock_service();
        // score=-2.5 → sigmoid(-2.5)=0.076, above empty(0.07) threshold.
        // kw_overlap=0 ("blahblah nothinghere" matches nothing), chunks.len()=1 < 2.
        // Condition: sigmoid < 0.12 && kw_overlap < 0.05 && chunks < 2 → LOW
        let weak_chunks = vec![mock_chunk("doc-1", -2.5, 0.005)];
        let report = service.calculate_confidence(
            "blahblah nothinghere faketerm",
            &weak_chunks,
            &RetrievalStrategy::Hybrid,
            &crate::domain::QueryComplexity::Simple,
        );
        assert_eq!(report.status, "LOW_CONFIDENCE_RETRIEVAL");
        assert_eq!(report.confidence, "low");
    }

    #[test]
    fn test_confidence_partial_retrieval() {
        let service = mock_service();
        // score=-1.5 → sigmoid(-1.5)=0.182, above LOW(0.12) but below PARTIAL(0.30).
        // Two chunks from different docs, kw_overlap for "test query" against empty
        // mock content is 0 → does not hit LOW (chunk count >= 2).
        // → PARTIAL_RETRIEVAL
        let partial_chunks = vec![
            mock_chunk("doc-1", -1.5, 0.02),
            mock_chunk("doc-2", -2.0, 0.02),
        ];
        let report = service.calculate_confidence(
            "test query",
            &partial_chunks,
            &RetrievalStrategy::Hybrid,
            &crate::domain::QueryComplexity::Simple,
        );
        assert_eq!(report.status, "PARTIAL_RETRIEVAL");
    }

    #[test]
    fn test_confidence_ok_high() {
        let service = mock_service();
        // sigmoid(3.0) = 0.952 > partial_threshold (0.50) → OK
        let strong_chunks = vec![
            mock_chunk("doc-1", 3.0, 0.03),
            mock_chunk("doc-1", 2.5, 0.02),
            mock_chunk("doc-1", 2.0, 0.02),
        ];
        let report = service.calculate_confidence(
            "test query",
            &strong_chunks,
            &RetrievalStrategy::Hybrid,
            &crate::domain::QueryComplexity::Simple,
        );
        assert_eq!(report.status, "OK");
        assert_eq!(report.confidence, "high");
        assert!(report.confidence_score >= 75);
    }

    // ── Ambiguity Tests (topic clustering) ────────────────────────────────────

    #[test]
    fn test_ambiguity_same_topic_is_not_ambiguous() {
        // Multiple Qdrant-related docs with a CLEAR score winner (gap > 0.12)
        // and same topic cluster → should NOT trigger AMBIGUOUS_RETRIEVAL.
        // sigmoid(3.0)=0.952, sigmoid(1.48)=0.814 → gap = 0.138 > 0.12
        // Both docs share qdrant, vector, database, deployment, embedding → same cluster
        let chunks = vec![
            mock_chunk_with_content(
                "doc-1",
                "qdrant vector database production",
                "Qdrant vector database production deployment embedding storage similarity search high performance tuning collection.",
                3.0,
            ),
            mock_chunk_with_content(
                "doc-2",
                "qdrant configuration guide",
                "Qdrant configuration guide vector database setup deployment performance tuning embedding collection similarity.",
                1.48,
            ),
        ];
        let result = is_ambiguous_retrieval(
            "Which vector database was selected for production and why",
            &chunks,
            &RetrievalStrategy::Hybrid,
        );
        assert!(!result, "Same-topic docs should NOT trigger AMBIGUOUS_RETRIEVAL");
    }

    #[test]
    fn test_ambiguity_different_topics_triggers_ambiguity() {
        // Authentication docs vs Qdrant docs with close scores → AMBIGUOUS
        let chunks = vec![
            mock_chunk_with_content("doc-1", "authentication_token_management", "JWT token authentication login session oauth credential management.", 1.5),
            mock_chunk_with_content("doc-2", "qdrant_vector_database", "Qdrant vector database embedding similarity search ANN retrieval.", 1.48),
        ];
        // "authentication" and "qdrant" share very few tokens → different clusters, close scores
        let result = is_ambiguous_retrieval("authentication qdrant", &chunks, &RetrievalStrategy::Hybrid);
        assert!(result, "Distinct-topic docs with close scores SHOULD trigger AMBIGUOUS_RETRIEVAL");
    }

    #[test]
    fn test_ambiguity_recursive_never_ambiguous() {
        let chunks = vec![
            mock_chunk("doc-1", 1.5, 0.02),
            mock_chunk("doc-2", 1.48, 0.02),
        ];
        assert!(!is_ambiguous_retrieval("some query", &chunks, &RetrievalStrategy::Recursive));
    }

    // ── Keyword Overlap Tests ─────────────────────────────────────────────────

    #[test]
    fn test_keyword_overlap_high_for_matching_content() {
        let chunks = vec![
            mock_chunk_with_content("doc-1", "authentication guide", "authentication token jwt oauth session credential login", 1.0),
        ];
        let score = keyword_overlap_score("explain authentication token management", &chunks);
        assert!(score > 0.1, "Should have meaningful overlap for auth query vs auth content");
    }

    #[test]
    fn test_keyword_overlap_zero_for_unrelated_content() {
        let chunks = vec![
            mock_chunk_with_content("doc-1", "qdrant database", "vector embedding similarity search ann cosine distance", 1.0),
        ];
        let score = keyword_overlap_score("explain authentication token", &chunks);
        // No tokens overlap between auth query and qdrant content
        assert!(score < 0.2, "Should have low overlap for unrelated content");
    }

    // ── Factual Anchor Extraction Tests ──────────────────────────────────────

    #[test]
    fn test_extract_factual_anchors_numbers() {
        let answer = "The RAG system uses 512 tokens per chunk with 10 percent overlap.";
        let anchors = extract_factual_anchors(answer);
        assert!(anchors.iter().any(|a| a.contains("512")), "Should extract number '512'");
    }

    #[test]
    fn test_extract_factual_anchors_proper_nouns() {
        let answer = "The system uses Qdrant for vector storage and Notion for document management.";
        let anchors = extract_factual_anchors(answer);
        assert!(anchors.iter().any(|a| a == "Qdrant"), "Should extract proper noun 'Qdrant'");
        assert!(anchors.iter().any(|a| a == "Notion"), "Should extract proper noun 'Notion'");
    }

    #[test]
    fn test_extract_factual_anchors_backtick() {
        let answer = "The config sets `chunk_size` to 512.";
        let anchors = extract_factual_anchors(answer);
        assert!(anchors.iter().any(|a| a.contains("chunk_size")), "Should extract backtick term");
    }

    // ── Fact Verification Tests ───────────────────────────────────────────────

    #[test]
    fn test_verify_grounded_answer_returns_high_coverage() {
        let chunks = vec![
            mock_chunk_with_content("doc-1", "RAG Config", "The RAG system uses 512 tokens per chunk with Qdrant as the vector database.", 1.0),
        ];
        let answer = "The RAG system uses 512 tokens per chunk. Qdrant is the vector database.";
        let coverage = verify_answer_against_chunks(answer, &chunks);
        assert!(coverage >= 0.5, "Grounded answer should have >= 50% fact coverage, got {}", coverage);
    }

    #[test]
    fn test_verify_hallucinated_answer_returns_low_coverage() {
        let chunks = vec![
            mock_chunk_with_content("doc-1", "RAG Config", "Simple chunk text without any specific technical terms.", 1.0),
        ];
        // All anchors are invented — none will be in the chunk
        let answer = "The system uses Pinecone vector database with Weaviate backup and 9999 tokens.";
        let coverage = verify_answer_against_chunks(answer, &chunks);
        assert!(coverage < 0.5, "Hallucinated answer should have < 50% fact coverage, got {}", coverage);
    }

    #[test]
    fn test_verify_no_anchors_returns_full_coverage() {
        let chunks = vec![mock_chunk("doc-1", 1.0, 0.5)];
        let answer = "This is a general statement without any specific facts.";
        let coverage = verify_answer_against_chunks(answer, &chunks);
        // No anchors → returns 1.0 (nothing to falsify)
        assert_eq!(coverage, 1.0);
    }

    // ── Recall Metrics Tests ──────────────────────────────────────────────────

    #[test]
    fn test_compute_recall_metrics_top_doc_unchanged() {
        let pre = vec![mock_chunk("doc-1", 1.0, 0.5), mock_chunk("doc-2", 0.8, 0.4)];
        let post = vec![mock_chunk("doc-1", 2.0, 0.5), mock_chunk("doc-2", 1.5, 0.4)];
        let recall = compute_recall_metrics(&pre, &post, 0.8);
        assert!(!recall.top_doc_changed, "Top doc should not have changed");
        assert_eq!(recall.unique_docs_pre_rerank, 2);
        assert_eq!(recall.unique_docs_post_rerank, 2);
        assert!((recall.fact_coverage - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn test_compute_recall_metrics_top_doc_changed() {
        let pre = vec![mock_chunk("doc-1", 1.0, 0.5), mock_chunk("doc-2", 0.8, 0.4)];
        // After reranking, doc-2 becomes top
        let post = vec![mock_chunk("doc-2", 3.0, 0.4), mock_chunk("doc-1", 1.5, 0.5)];
        let recall = compute_recall_metrics(&pre, &post, 0.6);
        assert!(recall.top_doc_changed, "Top doc SHOULD have changed after reranking");
    }
}
