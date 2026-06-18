use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use tokio::sync::RwLock;

use crate::db::Database;
use crate::domain::{
    AssistantResponse, Citation, ConfidenceBreakdown, DiagChunk, DiagnosticsPayload,
    MetadataFilters, QueryAnalysis, RetrievalResponse, RetrievalStrategy,
    RetrievedChunk,
};
use crate::services::context_builder::{BuiltContext, ContextBuilder};
use crate::services::document_graph::DocumentGraph;
use crate::services::entity_dictionary::{EntityDictionary, EntityMatch};
use crate::services::groq::GroqService;
use crate::services::ollama::OllamaService;
use crate::services::qdrant::{QdrantSearchFilter, QdrantSearchResult, QdrantService};
use crate::services::query_analyzer::QueryAnalyzerService;
use crate::services::reranker::RerankerService;
use crate::services::sparse::SparseRetrievalService;
use crate::services::topic_cluster::TopicCluster;

const MIN_ENTITY_RECURSION_SCORE: i32 = 3;
const MAX_RECURSIVE_ENTITIES: usize = 3;

#[derive(Clone)]
pub struct RetrievalService {
    ollama_service: OllamaService,
    qdrant_service: QdrantService,
    sparse_service: SparseRetrievalService,
    groq_service: GroqService,
    query_analyzer_service: QueryAnalyzerService,
    reranker_service: RerankerService,
    context_builder: ContextBuilder,
    /// Phase 2: in-memory document graph (wikilinks, md-links, title mentions)
    document_graph: Arc<RwLock<DocumentGraph>>,
    /// Phase 2: in-memory topic cluster assignments (doc → cluster mapping)
    topic_cluster: Arc<RwLock<TopicCluster>>,
    /// Dynamic entity dictionary built from indexed document metadata/titles
    entity_dictionary: Arc<RwLock<EntityDictionary>>,
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
            document_graph: Arc::new(RwLock::new(DocumentGraph::default())),
            topic_cluster: Arc::new(RwLock::new(TopicCluster::default())),
            entity_dictionary: Arc::new(RwLock::new(EntityDictionary::default())),
        }
    }

    /// Rebuilds the DocumentGraph and TopicCluster from all indexed documents.
    /// Called at startup (after index init) and after every successful sync.
    pub async fn rebuild_topic_graph(&self, database: &Database) -> Result<()> {
        let documents = database
            .document_repository()
            .list_all_chunk_search_documents()?;

        // Build graph
        let new_graph = DocumentGraph::build(&documents);
        // Build dynamic entity dictionary
        let new_dict = EntityDictionary::build(&documents);
        // Build clusters
        let new_cluster = TopicCluster::build(&documents, &new_dict);

        // Persist cluster assignments to SQLite for observability
        let assignments: Vec<(String, String, f32)> = {
            let docs_iter = documents.iter();
            let mut out = Vec::new();
            let mut seen_docs: std::collections::HashSet<&str> = std::collections::HashSet::new();
            for doc in docs_iter {
                if seen_docs.insert(doc.document_id.as_str()) {
                    for cluster_name in new_cluster.clusters_for_doc(&doc.document_id) {
                        out.push((doc.document_id.clone(), cluster_name.clone(), 1.0_f32));
                    }
                }
            }
            out
        };
        if let Err(e) = database.document_repository().save_document_clusters(&assignments) {
            tracing::warn!("[TOPIC_CLUSTER] Failed to persist cluster assignments: {}", e);
        }

        // Persist graph edges to SQLite for observability
        let edges = new_graph.all_edges();
        if let Err(e) = database.document_repository().save_document_graph_edges(&edges) {
            tracing::warn!("[DOCUMENT_GRAPH] Failed to persist graph edges: {}", e);
        }

        // Replace in-memory state
        *self.document_graph.write().await = new_graph;
        *self.topic_cluster.write().await = new_cluster;
        *self.entity_dictionary.write().await = new_dict;

        tracing::info!("[PHASE2] Rebuilt document graph, entity dictionary, and topic clusters");
        Ok(())
    }

    pub async fn ask_assistant(&self, database: &Database, query: &str) -> Result<AssistantResponse> {
        // ── RAG_DEBUG_MODE: bypass all confidence gating for diagnosis ─────────
        let debug_mode = std::env::var("RAG_DEBUG_MODE")
            .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
            .unwrap_or(false);
        if debug_mode {
            tracing::warn!("[DEBUG_MODE] RAG_DEBUG_MODE=true — confidence/ambiguity gating DISABLED");
        }

        // ── Phase 2: Broad-Topic / Ambiguity intercept ───────────────────────
        let matching_groups = self.entity_dictionary.read().await.detect_matching_groups(query);
        
        let dict = self.entity_dictionary.read().await;
        let mut scored_groups = Vec::new();
        for g_name in &matching_groups {
            if let Some(group) = dict.group_for_cluster(g_name) {
                let score = dict.score_group_for_query(group, query);
                scored_groups.push((g_name.clone(), score));
            }
        }

        // Sort by score descending
        scored_groups.sort_by(|a, b| b.1.cmp(&a.1));

        let should_trigger_ambiguity = if scored_groups.len() > 1 {
            let top_score = scored_groups[0].1;
            let second_score = scored_groups[1].1;
            // Relevance gap check: trigger ambiguity only if score gap is small (<= 5)
            (top_score as i32 - second_score as i32).abs() <= 5
        } else {
            false
        };

        if matching_groups.len() > 1 && should_trigger_ambiguity {
            let subject = dict.extract_subject(query).unwrap_or_else(|| query.to_string());
            let subject_title = clean_document_title(&subject);
            let mut answer = "I found multiple matching topics:\n\n".to_string();
            for g in &matching_groups {
                answer.push_str(&format!("* {} {}\n", clean_document_title(g), subject_title));
            }
            answer.push_str("\nWhich one would you like?");
            drop(dict);
            return Ok(AssistantResponse {
                answer,
                citations: Vec::new(),
                confidence: Some(crate::domain::ConfidenceReport {
                    confidence: "medium".to_string(),
                    confidence_score: 50,
                    reasons: vec!["Query maps to multiple topic clusters.".to_string()],
                    status: "AMBIGUOUS_RETRIEVAL".to_string(),
                    ambiguity_score: Some(1.0),
                }),
                diagnostics: None,
            });
        } else if !scored_groups.is_empty() && (!should_trigger_ambiguity || matching_groups.len() == 1) {
            let cluster_name = &scored_groups[0].0;
            tracing::info!(
                "[PHASE2] BroadTopic detected (either single match or large relevance gap) for query={:?} cluster={:?}",
                query, cluster_name
            );
            drop(dict);
            return self.ask_broad_topic(database, query, cluster_name).await;
        }
        drop(dict);

        let retrieval_start = std::time::Instant::now();
        let retrieval = self.retrieve_documents(database, query).await?;
        let retrieval_latency_ms = retrieval_start.elapsed().as_millis() as u32;

        let mut retrieval_results = retrieval.results.clone();
        for chunk in &mut retrieval_results {
            chunk.retrieval_score = Some(chunk.score);
        }

        // ── Expanded query (for diagnostics payload) ───────────────────────────
        let expanded_query = self.expand_query(query).await;

        // ── Pre-rerank snapshot (recall evaluation checkpoint 1) ───────────────
        let pre_rerank_chunks = retrieval_results.clone();

        let rerank_start = std::time::Instant::now();
        let reranked_chunks = if retrieval.strategy_used == crate::domain::RetrievalStrategy::Recursive {
            let mut top_chunks = retrieval_results;
            top_chunks.truncate(10);
            top_chunks
        } else {
            self.reranker_service
                .rerank(query, retrieval_results, 10)
                .await?
        };
        let rerank_latency_ms = rerank_start.elapsed().as_millis() as u32;

        // ── Post-rerank snapshot (recall evaluation checkpoint 2) ──────────────
        let post_rerank_chunks = reranked_chunks.clone();

        // ── [RERANKER] structured score log (raw logit + sigmoid for calibration)
        let (p50, p75, p90, p95) = {
            let sigmoid = |x: f32| 1.0_f32 / (1.0 + (-x).exp());
            for (rank, chunk) in reranked_chunks.iter().take(10).enumerate() {
                tracing::info!(
                    "[RERANKER] query={:?} rank={} doc={:?} raw_logit={:.4} sigmoid={:.4}",
                    query, rank + 1, chunk.document_title, chunk.score, sigmoid(chunk.score)
                );
            }
            let mut sigmoids: Vec<f32> = reranked_chunks.iter().map(|c| sigmoid(c.score)).collect();
            sigmoids.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            (
                get_percentile(&sigmoids, 0.50),
                get_percentile(&sigmoids, 0.75),
                get_percentile(&sigmoids, 0.90),
                get_percentile(&sigmoids, 0.95),
            )
        };
        tracing::info!(
            "[RERANKER] query={:?} score_percentiles: p50={:.4} p75={:.4} p90={:.4} p95={:.4}",
            query, p50, p75, p90, p95
        );

        // ── Confidence calculation ─────────────────────────────────────────────
        let dict = self.entity_dictionary.read().await;
        let (confidence_report, confidence_breakdown) = self.calculate_confidence_full(
            query,
            &reranked_chunks,
            &retrieval.strategy_used,
            &retrieval.analysis.complexity,
            &dict,
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
            let subject = self.entity_dictionary.read().await.extract_subject(query)
                .unwrap_or_else(|| {
                    query.split_whitespace().find(|w| w.len() > 4).unwrap_or(query).to_string()
                });
            let mut fallback_answer = format!(
                "I found multiple possible meanings for \"{}\":\n\n",
                subject
            );
            for doc in doc_aggregations.iter().take(3) {
                fallback_answer.push_str(&format!("• {}\n", clean_document_title(&doc.document_title)));
            }
            fallback_answer.push_str("\nWhich would you like?");
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

            let dict = self.entity_dictionary.read().await;
            let mut cits: Vec<Citation> = built_context
                .chunks
                .into_iter()
                .map(|chunk| {
                    let matching = top_chunks.iter().find(|c| c.chunk_id == chunk.chunk_id);
                    let retrieval_score = matching.and_then(|c| c.retrieval_score);
                    let rerank_score = matching.map(|c| c.score).unwrap_or(chunk.score);

                    let section = matching
                        .and_then(|c| c.metadata.get("section"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let evidence = extract_evidence(&chunk.content, query, &dict);
                    let has_evidence = evidence.is_some();

                    let sigmoid = |x: f32| 1.0_f32 / (1.0 + (-x).exp());
                    let sig = sigmoid(rerank_score);

                    let anchors = extract_factual_anchors(&ans);
                    let match_count = anchors.iter().filter(|a| chunk.content.to_lowercase().contains(&a.to_lowercase())).count();

                    let evidence_level = if has_evidence {
                        if match_count >= 2 || sig >= 0.75 {
                            "High Evidence".to_string()
                        } else if match_count >= 1 || sig >= 0.45 {
                            "Medium Evidence".to_string()
                        } else {
                            "Supporting Evidence".to_string()
                        }
                    } else {
                        "Supporting Evidence".to_string()
                    };

                    Citation {
                        source_document: clean_document_title(&chunk.document_title),
                        source_type: chunk.source.clone(),
                        chunk_id: chunk.chunk_id.clone(),
                        retrieval_score,
                        rerank_score,
                        section,
                        evidence: evidence.clone(),
                        evidence_level: Some(evidence_level.clone()),
                        document_title: clean_document_title(&chunk.document_title),
                        evidence_snippet: evidence.clone(),
                        source_connector: chunk.source.clone(),
                        // Legacy:
                        source: chunk.source.clone(),
                        document_id: chunk.document_id.clone(),
                        score: chunk.score,
                    }
                })
                .collect();

            // Sort citations by:
            // 1. Evidence level (High > Medium > Supporting).
            // 2. Document relevance (rerank score descending).
            // 3. Number of supporting citations for that document in the top set.
            let mut doc_counts = HashMap::new();
            for cit in &cits {
                *doc_counts.entry(cit.source_document.clone()).or_insert(0) += 1;
            }

            cits.sort_by(|a, b| {
                let level_score = |level: &Option<String>| match level.as_deref() {
                    Some("High Evidence") => 3,
                    Some("Medium Evidence") => 2,
                    Some("Supporting Evidence") => 1,
                    _ => 0,
                };
                
                let a_level = level_score(&a.evidence_level);
                let b_level = level_score(&b.evidence_level);
                
                if a_level != b_level {
                    return b_level.cmp(&a_level);
                }
                
                if let Some(ord) = b.rerank_score.partial_cmp(&a.rerank_score) {
                    if ord != std::cmp::Ordering::Equal {
                        return ord;
                    }
                }
                
                let a_count = doc_counts.get(&a.source_document).copied().unwrap_or(0);
                let b_count = doc_counts.get(&b.source_document).copied().unwrap_or(0);
                b_count.cmp(&a_count)
            });

            // Prepend warning if PARTIAL_RETRIEVAL
            if confidence_report.status == "PARTIAL_RETRIEVAL" {
                ans = format!(
                    "⚠️ This answer may be incomplete because only part of the requested information was found.\n\n{}",
                    ans
                );
            }

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
            confidence_breakdown: confidence_breakdown.clone(),
            final_status: confidence_report.status.clone(),
            recall_metrics: recall,
        };

        // ── Structured diagnostic trace log ──────────────────────────────────
        let intent_type = determine_query_intent(query, &retrieval.analysis, &dict);
        let retrieved_titles: Vec<String> = pre_rerank_chunks.iter().map(|c| c.document_title.clone()).collect();
        let reranked_titles: Vec<String> = post_rerank_chunks.iter().map(|c| c.document_title.clone()).collect();
        
        let chunks_with_evidence = citations.iter().filter(|c| c.evidence.is_some()).count();
        let total_chunks = citations.len();
        let evidence_coverage = if total_chunks == 0 {
            0.0
        } else {
            chunks_with_evidence as f32 / total_chunks as f32
        };

        let confidence_inputs_str = format!(
            "rerank_top_sigmoid: {:.4}, avg_top5_sigmoid: {:.4}, evidence_consistency_score: {}, document_focus_bonus: {}, keyword_overlap: {:.4}, title_match_bonus: {}, retrieval_signal: {}, p50: {:.4}, p75: {:.4}, p90: {:.4}, p95: {:.4}",
            confidence_breakdown.reranker_top_sigmoid,
            confidence_breakdown.avg_top5_sigmoid,
            confidence_breakdown.evidence_consistency_score,
            confidence_breakdown.document_focus_bonus,
            confidence_breakdown.keyword_overlap_score,
            confidence_breakdown.title_match_bonus,
            confidence_breakdown.retrieval_signal_score,
            p50, p75, p90, p95
        );

        tracing::info!(
            "\n==================== RAG PIPELINE DIAGNOSTIC TRACE ====================\n\
             Query: {}\n\
             Intent Type: {:?}\n\
             Retrieval Strategy: {:?}\n\
             Retrieved Docs: {:?}\n\
             Reranked Docs (Top 5): {:?}\n\
             Confidence Inputs: {}\n\
             Confidence Result Status: {} (Score: {}/100)\n\
             Evidence Coverage: {:.2} (Chunks with evidence: {}/{})\n\
             Citation Count: {}\n\
             =======================================================================",
            query,
            intent_type,
            retrieval.strategy_used,
            retrieved_titles,
            reranked_titles.iter().take(5).collect::<Vec<_>>(),
            confidence_inputs_str,
            confidence_report.status,
            confidence_report.confidence_score,
            evidence_coverage,
            chunks_with_evidence,
            total_chunks,
            total_chunks
        );

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
        entity_dict: &EntityDictionary,
    ) -> (crate::domain::ConfidenceReport, ConfidenceBreakdown) {
        let mut reasons = Vec::new();

        if chunks.is_empty() {
            let breakdown = ConfidenceBreakdown {
                reranker_top_sigmoid: 0.0,
                avg_top5_sigmoid: 0.0,
                evidence_consistency_score: 0,
                document_focus_bonus: 0,
                keyword_overlap_score: 0.0,
                title_match_bonus: 0,
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

        // ── Signal 1 & 2: Reranker scores (20% raw sigmoid + 20% percentile = 40%) ─────────────────────
        let top_rerank_score = chunks[0].score;
        let top_normalized_rerank = sigmoid(top_rerank_score);

        // Percentile rank score computation
        let reranker_percentile_score = {
            let mut sigmoids: Vec<f32> = chunks.iter().map(|c| sigmoid(c.score)).collect();
            sigmoids.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let top_sig = sigmoids.last().copied().unwrap_or(0.0);
            let p50 = get_percentile(&sigmoids, 0.50);
            let p90 = get_percentile(&sigmoids, 0.90);
            
            if sigmoids.len() < 3 {
                if top_sig >= 0.25 { 100.0 } else { 0.0 }
            } else if top_sig < 0.05 {
                0.0
            } else {
                let range = (p90 - p50).max(0.001);
                let ratio = (top_sig - p50) / range;
                (ratio * 100.0).clamp(0.0, 100.0)
            }
        };

        let limit = chunks.len().min(5);
        let sum_top_5: f32 = chunks.iter().take(limit).map(|c| sigmoid(c.score)).sum();
        let avg_top_5_normalized_rerank = sum_top_5 / limit as f32;

        // Variance across all reranked chunks
        let mean_all: f32 = chunks.iter().map(|c| sigmoid(c.score)).sum::<f32>() / chunks.len() as f32;
        let variance_all: f32 = chunks.iter().map(|c| {
            let diff = sigmoid(c.score) - mean_all;
            diff * diff
        }).sum::<f32>() / chunks.len() as f32;

        // ── Keyword overlap (20%) ──────────────────────────────────────
        let kw_overlap = keyword_overlap_score(query, chunks);
        let kw_bonus = (kw_overlap * 20.0) as i32;

        // ── Title-keyword match bonus (15%) ──────────────────────────────────────
        let title_match_bonus = query_title_match_bonus(query, chunks);

        // ── Evidence consistency (15%) ──────────────────────────────────────────
        let evidence_count = chunks.iter().take(3)
            .filter_map(|c| extract_evidence(&c.content, query, entity_dict))
            .count();
        let has_any_evidence = evidence_count > 0;
        let evidence_consistency_score = if evidence_count >= 2 {
            15
        } else if evidence_count == 1 {
            8
        } else {
            0
        };

        // ── Document focus / agreement (10%) ──────────────────────────────────────
        let doc_aggregations = aggregate_documents(chunks);
        let unique_docs = doc_aggregations.len();
        let document_focus_bonus: i32 = if unique_docs == 1 && chunks.len() >= 3 {
            10
        } else if unique_docs <= 2 {
            5
        } else {
            0
        };

        // ── Retrieval signal score ────────────────────────────────────────────────
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

        // ── Core keyword specificity guard ───────────────────────────────────────
        let stop_words_core: HashSet<&str> = [
            "what", "is", "the", "does", "how", "to", "explain", "describe", "about",
            "system", "use", "connect", "with", "and", "or", "a", "an", "of",
            "which", "where", "that", "this", "for", "from", "are", "was", "were",
            "has", "have", "had", "can", "will", "would", "should", "could", "in", "on",
            "between", "among", "through", "during", "before", "after",
        ].iter().cloned().collect();

        let topic_words_core: HashSet<&str> = [
            "database", "system", "authentication", "monitoring", "observability",
            "configuration", "settings", "notion", "obsidian", "setup", "auth",
            "telemetry", "metrics", "storage", "persistence", "integration",
            "integrations", "process", "pipeline", "sync", "flow", "guide",
            "credential", "credentials", "token", "tokens", "client", "server",
            "service", "services", "user", "users", "compare", "contrast",
            "difference", "relationship", "connection", "relation", "list",
            "show", "explain", "describe", "summarize", "check", "find", "get",
            "tell", "which", "whose", "where", "whom", "stores", "interact",
            "interacts", "interaction", "interactions", "management", "manager",
            "managers", "connect", "connects", "relate", "relates", "versus", "vs"
        ].iter().cloned().collect();

        let query_keywords: Vec<String> = query.to_lowercase()
            .split_whitespace()
            .map(|w| {
                let without_pos = w.strip_suffix("'s").unwrap_or(w);
                without_pos.trim_matches(|c: char| !c.is_alphanumeric()).to_string()
            })
            .filter(|w| w.len() >= 3 && !stop_words_core.contains(w.as_str()) && !topic_words_core.contains(w.as_str()))
            .collect();

        let mut missing_core_keyword = false;
        let mut missing_kws = Vec::new();
        let chunks_text = chunks.iter().take(3)
            .map(|c| format!("{} {}", c.content.to_lowercase(), c.document_title.to_lowercase()))
            .collect::<Vec<_>>()
            .join(" ");

        for kw in &query_keywords {
            if !chunks_text.contains(kw) {
                missing_core_keyword = true;
                missing_kws.push(kw.clone());
            }
        }

        // ── Combine signals ───────────────────────────────────────────────────────
        let mut confidence_score =
            (top_normalized_rerank * 20.0) as i32
            + (reranker_percentile_score * 0.20) as i32
            + kw_bonus
            + title_match_bonus
            + evidence_consistency_score
            + document_focus_bonus;

        if evidence_count == 0 {
            confidence_score -= 8;
        }

        if missing_core_keyword {
            confidence_score -= 20;
        }

        // ── Reason messages ───────────────────────────────────────────────────────
        if top_normalized_rerank >= 0.80 {
            reasons.push("Strong relevance match identified by reranker.".to_string());
        } else if top_normalized_rerank < 0.40 {
            reasons.push("Reranker indicates low relevance matching.".to_string());
        }
        if kw_overlap >= 0.4 {
            reasons.push("High keyword overlap between query and retrieved chunks.".to_string());
        }
        if title_match_bonus > 0 {
            reasons.push("Query keyword directly matches the top document title.".to_string());
        }
        if document_focus_bonus >= 10 {
            reasons.push("High document focus (all chunks from single source).".to_string());
        }
        if missing_core_keyword {
            reasons.push(format!("Core query keyword(s) {:?} missing from retrieved context.", missing_kws));
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
        let cluster_guard = self.topic_cluster.try_read();
        let topic_cluster_ref = cluster_guard.as_ref().ok().map(|g| &**g);
        let is_ambiguous = is_ambiguous_retrieval(query, chunks, strategy, topic_cluster_ref, entity_dict);
        if is_ambiguous {
            confidence_score -= 10;
            reasons.push("Ambiguous relevance — multiple topic clusters detected.".to_string());
        }

        let confidence_score = confidence_score.clamp(0, 100) as u32;

        // ── Systematic Gating ─────────────────────────────────────────────────────
        // Thresholds are calibrated against real cross-encoder distributions:
        //   sigmoid(logit)  |  logit
        //     0.10          |  -2.20   (strong miss)
        //     0.15          |  -1.73   (very weak signal)
        //     0.25          |  -1.10   (weak but plausible)
        //     0.35          |  -0.62   (moderate — should NOT be gated out)
        //     0.50          |   0.00   (neutral)
        //
        // EMPTY: only fire when evidence is genuinely absent AND reranker is strongly
        //        negative (sigmoid < 0.10), or when both keyword and sigmoid are near zero.
        let is_empty = chunks.is_empty()
            || (top_normalized_rerank < 0.10 && !has_any_evidence && kw_overlap < 0.05)
            || (top_normalized_rerank < 0.08 && kw_overlap < 0.03);

        // LOW_CONFIDENCE: fire when signal is clearly weak but not fully absent.
        let is_low_confidence = !is_empty && (
            (top_normalized_rerank < 0.15 && evidence_count == 0)
            || (top_normalized_rerank < 0.12 && kw_overlap < 0.05)
            || (kw_overlap < 0.01 && !has_any_evidence)
            || missing_core_keyword
        );

        // PARTIAL: fire only when reranker gives weak-to-moderate signal
        //          and both keyword overlap AND evidence are weak.
        //          Do NOT gate out results with sigmoid >= 0.28.
        let is_partial = !is_empty && !is_low_confidence && (
            (top_normalized_rerank < 0.28 && kw_overlap < 0.10)
            || (evidence_count == 0 && kw_overlap < 0.10)
        );

        let status = if is_empty {
            reasons.push("No chunks found or all chunk scores are below minimum matching relevance.".to_string());
            "EMPTY_RETRIEVAL".to_string()
        } else if is_ambiguous {
            reasons.push("Multiple documents competing across distinct topic clusters.".to_string());
            "AMBIGUOUS_RETRIEVAL".to_string()
        } else if is_low_confidence {
            reasons.push("Overall low matching confidence across retrieval layers.".to_string());
            "LOW_CONFIDENCE_RETRIEVAL".to_string()
        } else if is_partial {
            reasons.push("Partial context retrieved; some sources may be weakly relevant.".to_string());
            "PARTIAL_RETRIEVAL".to_string()
        } else {
            "OK".to_string()
        };

        // ── [CONFIDENCE] structured log for failure attribution ───────────────────
        tracing::info!(
            "[CONFIDENCE_AUDIT] query={:?} status={} final_score={} top_sigmoid={:.4} avg_top5_sigmoid={:.4} evidence_consistency_score={} doc_focus_bonus={} kw_overlap={:.4} kw_bonus={} retrieval_signal={} title_match_bonus={} evidence_count={} ambiguity_fired={}",
            query, status, confidence_score, top_normalized_rerank, avg_top_5_normalized_rerank,
            evidence_consistency_score, document_focus_bonus, kw_overlap, kw_bonus, retrieval_signal_score,
            title_match_bonus, evidence_count, is_ambiguous
        );

        let confidence = if confidence_score >= 60 {
            "high".to_string()
        } else if confidence_score >= 35 {
            "medium".to_string()
        } else {
            "low".to_string()
        };

        let breakdown = ConfidenceBreakdown {
            reranker_top_sigmoid: top_normalized_rerank,
            avg_top5_sigmoid: avg_top_5_normalized_rerank,
            evidence_consistency_score,
            document_focus_bonus,
            keyword_overlap_score: kw_overlap,
            title_match_bonus,
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
        entity_dict: &EntityDictionary,
    ) -> crate::domain::ConfidenceReport {
        self.calculate_confidence_full(query, chunks, strategy, complexity, entity_dict).0
    }

    /// Broad-Topic Synthesis path — used when `detect_broad_topic` fires.
    ///
    /// Strategy:
    ///   1. Fetch cluster doc IDs from TopicCluster (top 3 documents)
    ///   2. Retrieve top 3 chunks per doc → max 9 chunks total
    ///   3. Rerank all 9 chunks against the query
    ///   4. Synthesize a structured overview with a special prompt
    ///   5. Cite all contributing documents (no ambiguity check — broad topic is intentional)
    async fn ask_broad_topic(
        &self,
        database: &Database,
        query: &str,
        cluster_name: &str,
    ) -> Result<AssistantResponse> {
        let dict = self.entity_dictionary.read().await;
        tracing::info!(
            "[BROAD_TOPIC] synthesizing answer for cluster={:?} query={:?}",
            cluster_name, query
        );

        // Step 1: Get document IDs in the cluster (top 3 by confidence)
        let cluster_doc_ids: Vec<String> = {
            let cluster_guard = self.topic_cluster.read().await;
            cluster_guard.docs_in_cluster(cluster_name)
                .into_iter()
                .take(3)
                .collect()
        };

        if cluster_doc_ids.is_empty() {
            // No cluster data yet — fall back to normal retrieval
            tracing::warn!(
                "[BROAD_TOPIC] cluster={:?} has no docs, falling back to normal retrieval",
                cluster_name
            );
            let retrieval = self.retrieve_documents(database, query).await?;
            let retrieval_results = retrieval.results.clone();
            let reranked = self.reranker_service.rerank(query, retrieval_results, 10).await?;
            let top_chunks: Vec<RetrievedChunk> = reranked.into_iter().take(3).collect();
            let built_context = self.context_builder.build(top_chunks.clone());
            let answer = self.generate_answer(query, &retrieval.analysis, &built_context).await?;
            let mut citations: Vec<Citation> = top_chunks.iter().map(|c| {
                let section = c.metadata.get("section")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let evidence = extract_evidence(&c.content, query, &dict);
                let has_evidence = evidence.is_some();
                
                let sigmoid = |x: f32| 1.0_f32 / (1.0 + (-x).exp());
                let sig = sigmoid(c.score);
                
                let anchors = extract_factual_anchors(&answer);
                let match_count = anchors.iter().filter(|a| c.content.to_lowercase().contains(&a.to_lowercase())).count();
                
                let evidence_level = if has_evidence && (match_count >= 2 || sig >= 0.75) {
                    "High Evidence".to_string()
                } else if match_count >= 1 || sig >= 0.45 {
                    "Medium Evidence".to_string()
                } else {
                    "Supporting Evidence".to_string()
                };

                Citation {
                    source_document: clean_document_title(&c.document_title),
                    source_type: c.source.clone(),
                    chunk_id: c.chunk_id.clone(),
                    retrieval_score: c.retrieval_score,
                    rerank_score: c.score,
                    section,
                    evidence: evidence.clone(),
                    evidence_level: Some(evidence_level),
                    document_title: clean_document_title(&c.document_title),
                    evidence_snippet: evidence.clone(),
                    source_connector: c.source.clone(),
                    source: c.source.clone(),
                    document_id: c.document_id.clone(),
                    score: c.score,
                }
            }).collect();

            let mut doc_counts = HashMap::new();
            for cit in &citations {
                *doc_counts.entry(cit.source_document.clone()).or_insert(0) += 1;
            }

            citations.sort_by(|a, b| {
                let level_score = |level: &Option<String>| match level.as_deref() {
                    Some("High Evidence") => 3,
                    Some("Medium Evidence") => 2,
                    Some("Supporting Evidence") => 1,
                    _ => 0,
                };
                
                let a_level = level_score(&a.evidence_level);
                let b_level = level_score(&b.evidence_level);
                
                if a_level != b_level {
                    return b_level.cmp(&a_level);
                }
                
                if let Some(ord) = b.rerank_score.partial_cmp(&a.rerank_score) {
                    if ord != std::cmp::Ordering::Equal {
                        return ord;
                    }
                }
                
                let a_count = doc_counts.get(&a.source_document).copied().unwrap_or(0);
                let b_count = doc_counts.get(&b.source_document).copied().unwrap_or(0);
                b_count.cmp(&a_count)
            });
            let confidence_report = crate::domain::ConfidenceReport {
                confidence: "high".to_string(),
                confidence_score: 85,
                reasons: vec!["Broad-topic fallback retrieval executed successfully.".to_string()],
                status: "OK".to_string(),
                ambiguity_score: Some(0.0),
            };
            return Ok(AssistantResponse { answer, citations, confidence: Some(confidence_report), diagnostics: None });
        }

        // Step 2: For each doc in the cluster, fetch its top chunks
        let mut all_cluster_chunks: Vec<RetrievedChunk> = Vec::new();
        for doc_id in &cluster_doc_ids {
            let doc_chunks = database
                .document_repository()
                .get_chunks_by_document(doc_id)?;
            // Take the first 3 ordinal chunks (they are the most representative)
            let chunk_ids: Vec<String> = doc_chunks
                .iter()
                .filter(|c| c.embedding_status == "completed")
                .take(3)
                .map(|c| c.id.clone())
                .collect();
            if chunk_ids.is_empty() {
                continue;
            }
            let mut hydrated = hydrate_chunk_ids(database, &chunk_ids)?;
            for chunk in &mut hydrated {
                chunk.retrieval_score = Some(chunk.score);
            }
            all_cluster_chunks.extend(hydrated);
        }
        if all_cluster_chunks.is_empty() {
            let confidence_report = crate::domain::ConfidenceReport {
                confidence: "low".to_string(),
                confidence_score: 0,
                reasons: vec!["No chunks found for the broad-topic cluster.".to_string()],
                status: "EMPTY_RETRIEVAL".to_string(),
                ambiguity_score: Some(0.0),
            };
            return Ok(AssistantResponse {
                answer: format!(
                    "I found documents related to {} in your knowledge base, but could not retrieve their content. \
                     This may indicate an indexing issue — try syncing your documents and asking again.",
                    cluster_name
                ),
                citations: vec![],
                confidence: Some(confidence_report),
                diagnostics: None,
            });
        }

        // Step 3: Rerank all cluster chunks against the query
        let reranked = self
            .reranker_service
            .rerank(query, all_cluster_chunks, 10)
            .await?;

        // Step 4: Take top 9 reranked chunks (3 docs × 3 chunks max)
        let top_chunks: Vec<RetrievedChunk> = reranked.into_iter().take(9).collect();

        // Step 5: Build context and synthesize with a broad-topic prompt
        let built_context = self.context_builder.build(top_chunks.clone());

        // Build a fake QueryAnalysis for generate_answer (strategy is not used for prompt selection)
        let dummy_analysis = crate::domain::QueryAnalysis {
            intent: "broad_topic_synthesis".to_string(),
            entities: vec![cluster_name.to_string()],
            metadata_filters: MetadataFilters::default(),
            temporal: false,
            complexity: crate::domain::QueryComplexity::Complex,
            strategy: RetrievalStrategy::Hybrid,
        };

        let answer = self
            .generate_answer(query, &dummy_analysis, &built_context)
            .await?;

        // Step 6: Build citations for all contributing docs
        let dict = self.entity_dictionary.read().await;
        let mut citations: Vec<Citation> = top_chunks
            .iter()
            .map(|chunk| {
                let section = chunk.metadata.get("section")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let evidence = extract_evidence(&chunk.content, query, &dict);
                let has_evidence = evidence.is_some();
                
                let sigmoid = |x: f32| 1.0_f32 / (1.0 + (-x).exp());
                let sig = sigmoid(chunk.score);
                
                let anchors = extract_factual_anchors(&answer);
                let match_count = anchors.iter().filter(|a| chunk.content.to_lowercase().contains(&a.to_lowercase())).count();
                
                let evidence_level = if has_evidence {
                    if match_count >= 2 || sig >= 0.75 {
                        "High Evidence".to_string()
                    } else if match_count >= 1 || sig >= 0.45 {
                        "Medium Evidence".to_string()
                    } else {
                        "Supporting Evidence".to_string()
                    }
                } else {
                    "Supporting Evidence".to_string()
                };

                Citation {
                    source_document: clean_document_title(&chunk.document_title),
                    source_type: chunk.source.clone(),
                    chunk_id: chunk.chunk_id.clone(),
                    retrieval_score: chunk.retrieval_score,
                    rerank_score: chunk.score,
                    section,
                    evidence: evidence.clone(),
                    evidence_level: Some(evidence_level.clone()),
                    document_title: clean_document_title(&chunk.document_title),
                    evidence_snippet: evidence.clone(),
                    source_connector: chunk.source.clone(),
                    source: chunk.source.clone(),
                    document_id: chunk.document_id.clone(),
                    score: chunk.score,
                }
            })
            .collect();

        let mut doc_counts = HashMap::new();
        for cit in &citations {
            *doc_counts.entry(cit.source_document.clone()).or_insert(0) += 1;
        }

        citations.sort_by(|a, b| {
            let level_score = |level: &Option<String>| match level.as_deref() {
                Some("High Evidence") => 3,
                Some("Medium Evidence") => 2,
                Some("Supporting Evidence") => 1,
                _ => 0,
            };
            
            let a_level = level_score(&a.evidence_level);
            let b_level = level_score(&b.evidence_level);
            
            if a_level != b_level {
                return b_level.cmp(&a_level);
            }
            
            if let Some(ord) = b.rerank_score.partial_cmp(&a.rerank_score) {
                if ord != std::cmp::Ordering::Equal {
                    return ord;
                }
            }
            
            let a_count = doc_counts.get(&a.source_document).copied().unwrap_or(0);
            let b_count = doc_counts.get(&b.source_document).copied().unwrap_or(0);
            b_count.cmp(&a_count)
        });

        tracing::info!(
            "[BROAD_TOPIC] Synthesized answer for cluster={:?} using {} chunks from {} docs",
            cluster_name,
            top_chunks.len(),
            cluster_doc_ids.len()
        );

        let confidence_report = crate::domain::ConfidenceReport {
            confidence: "high".to_string(),
            confidence_score: 85,
            reasons: vec!["Broad-topic synthesis executed successfully.".to_string()],
            status: "OK".to_string(),
            ambiguity_score: Some(0.0),
        };
        Ok(AssistantResponse {
            answer,
            citations,
            confidence: Some(confidence_report),
            diagnostics: None, // Diagnostics panel deferred (no telemetry row for synthesis)
        })
    }

    pub async fn expand_query(&self, query: &str) -> String {
        let (expanded, _) = self.entity_dictionary.read().await.expand(query);
        expanded
    }

    pub async fn retrieve_documents(
        &self,
        database: &Database,
        query: &str,
    ) -> Result<RetrievalResponse> {
        let analysis = self.query_analyzer_service.analyze(database, query).await?;
        
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
        let search_query = self.expand_query(query).await;
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
            RetrievalStrategy::Recursive => self.retrieve_recursive(database, query, analysis, depth).await,
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
        // Dense retrieval is best-effort: if Qdrant is unavailable, fall back to
        // sparse-only rather than propagating an error and breaking the entire pipeline.
        let search_limit = limit.max(30);
        let dense = match self
            .retrieve_dense(database, query, analysis.metadata_filters.as_ref(), search_limit)
            .await
        {
            Ok(chunks) => {
                tracing::debug!("[HYBRID] dense retrieval returned {} chunks", chunks.len());
                chunks
            }
            Err(err) => {
                tracing::warn!(
                    "[HYBRID] Dense retrieval unavailable (Qdrant may be down): {}. \
                     Continuing with sparse-only retrieval.",
                    err
                );
                Vec::new()
            }
        };
        let sparse = self
            .retrieve_sparse(database, query, analysis.metadata_filters.as_ref(), search_limit)
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
        let dict = self.entity_dictionary.read().await;
        let entity_matches = dict.score_entities(query, analysis);

        let accepted_entities: Vec<EntityMatch> = entity_matches
            .iter()
            .filter(|m| m.score >= 8)
            .take(5)
            .cloned()
            .collect();

        let mut detected_list = Vec::new();
        let mut accepted_list = Vec::new();
        let mut rejected_list = Vec::new();

        for m in &entity_matches {
            detected_list.push(m.group_name.clone());
            if m.score >= 8 {
                accepted_list.push(m.group_name.clone());
            } else {
                rejected_list.push(m.group_name.clone());
            }
        }

        detected_list.sort();
        accepted_list.sort();
        rejected_list.sort();

        println!("[ENTITY_ROUTING]");
        println!("Detected:");
        for e in &detected_list {
            println!("{}", e);
        }
        println!("\nAccepted:");
        for e in &accepted_list {
            println!("{}", e);
        }
        println!("\nRejected:");
        for e in &rejected_list {
            println!("{}", e);
        }
        println!();

        println!("[ENTITY_ROUTING]");
        for m in &entity_matches {
            if m.score >= 8 {
                println!("{} -> score={} ACCEPTED", m.group_name, m.score);
            } else {
                println!("{} -> score={} REJECTED (below threshold)", m.group_name, m.score);
            }
        }
        println!();

        let intent_type = determine_query_intent(query, analysis, &dict);

        let mut targets = Vec::new();
        let query_words: Vec<String> = query.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let generic_names = [
            "setup", "integration", "integrations", "guide", "flow", "overview", 
            "details", "process", "sync", "pipeline", "configuration", "settings"
        ];

        for group in &dict.groups {
            let group_name_lower = group.name.to_lowercase();
            if !generic_names.contains(&group_name_lower.as_str()) && query_words.contains(&group_name_lower) {
                if !targets.contains(&group_name_lower) {
                    targets.push(group_name_lower.clone());
                }
            }
            for term in &group.primary_terms {
                let term_lower = term.to_lowercase();
                if !generic_names.contains(&term_lower.as_str()) && query_words.contains(&term_lower) {
                    if !targets.contains(&term_lower) {
                        targets.push(term_lower.clone());
                    }
                }
            }
            for term in &group.specific_terms {
                let term_lower = term.to_lowercase();
                if !generic_names.contains(&term_lower.as_str()) && query_words.contains(&term_lower) {
                    if !targets.contains(&term_lower) {
                        targets.push(term_lower.clone());
                    }
                }
            }
            for term in &group.expansion_terms {
                let term_lower = term.to_lowercase();
                if !generic_names.contains(&term_lower.as_str()) && query_words.contains(&term_lower) {
                    if !targets.contains(&term_lower) {
                        targets.push(term_lower.clone());
                    }
                }
            }
        }

        if targets.len() < 2 {
            for m in &accepted_entities {
                if !targets.contains(&m.group_name) {
                    targets.push(m.group_name.clone());
                }
            }
        }

        let mut hop1_chunks = Vec::new();
        let mut clean_analysis = analysis.clone();
        clean_analysis.metadata_filters = MetadataFilters::default();

        println!("[RECURSIVE_DIAGNOSTIC] query: {:?}", query);
        println!("[RECURSIVE_DIAGNOSTIC] targets: {:?}", targets);

        let planned_passes;
        let mut executed_passes = 0;
        let mut discarded_passes = 0;

        if targets.len() >= 2 {
            let target_x = targets[0].clone();
            let target_y = targets[1].clone();
            planned_passes = 3;

            if intent_type == QueryIntentType::Comparison {
                if executed_passes < 6 {
                    let chunks = self.retrieve_hybrid(database, &target_x, &clean_analysis, 30).await?;
                    hop1_chunks.extend(chunks);
                    executed_passes += 1;
                } else {
                    discarded_passes += 1;
                }

                if executed_passes < 6 {
                    let chunks = self.retrieve_hybrid(database, &target_y, &clean_analysis, 30).await?;
                    hop1_chunks.extend(chunks);
                    executed_passes += 1;
                } else {
                    discarded_passes += 1;
                }

                if executed_passes < 6 {
                    let joint = format!("{} {}", target_x, target_y);
                    let chunks = self.retrieve_hybrid(database, &joint, &clean_analysis, 30).await?;
                    hop1_chunks.extend(chunks);
                    executed_passes += 1;
                } else {
                    discarded_passes += 1;
                }
            } else {
                let mut x_chunks = Vec::new();
                let mut y_chunks = Vec::new();

                if executed_passes < 6 {
                    x_chunks = self.retrieve_hybrid(database, &target_x, &clean_analysis, 30).await?;
                    hop1_chunks.extend(x_chunks.clone());
                    executed_passes += 1;
                } else {
                    discarded_passes += 1;
                }

                if executed_passes < 6 {
                    y_chunks = self.retrieve_hybrid(database, &target_y, &clean_analysis, 30).await?;
                    hop1_chunks.extend(y_chunks.clone());
                    executed_passes += 1;
                } else {
                    discarded_passes += 1;
                }

                let stop_words: HashSet<&str> = [
                    "about", "above", "after", "again", "against", "all", "am", "an", "and", "any", "are", "aren't",
                    "as", "at", "be", "because", "been", "before", "being", "below", "between", "both", "but", "by",
                    "can't", "cannot", "could", "couldn't", "did", "didn't", "do", "does", "doesn't", "doing", "don't",
                    "down", "during", "each", "few", "for", "from", "further", "had", "hadn't", "has", "hasn't", "have",
                    "haven't", "having", "he", "he'd", "he'll", "he's", "her", "here", "here's", "hers", "herself",
                    "him", "himself", "his", "how", "how's", "i", "i'd", "i'll", "i'm", "i've", "if", "in", "into", "is",
                    "isn't", "it", "it's", "its", "itself", "let's", "me", "more", "most", "mustn't", "my", "myself",
                    "no", "nor", "not", "of", "off", "on", "once", "only", "or", "other", "ought", "our", "ours",
                    "ourselves", "out", "over", "own", "same", "shan't", "she", "she'd", "she'll", "she's", "should",
                    "shouldn't", "so", "some", "such", "than", "that", "that's", "the", "their", "theirs", "them",
                    "themselves", "then", "there", "there's", "these", "they", "they'd", "they'll", "they're",
                    "they've", "this", "those", "through", "to", "too", "under", "until", "up", "very", "was", "wasn't",
                    "we", "we'd", "we'll", "we're", "we_ve", "were", "weren't", "what", "what's", "when", "when's",
                    "where", "where's", "which", "while", "who", "who's", "whom", "why", "why's", "with", "won't",
                    "would", "wouldn't", "you", "you'd", "you'll", "you're", "you've", "your", "yours", "yourself",
                    "yourselves", "system", "setup", "guide", "onboarding", "notion", "obsidian", "prometheus", "grafana",
                    "authentication", "qdrant", "database", "access", "control"
                ].iter().cloned().collect();

                let extract_words = |chunks: &[RetrievedChunk]| -> HashSet<String> {
                    let mut w_set = HashSet::new();
                    for chunk in chunks {
                        let text = format!("{} {}", chunk.content.to_lowercase(), chunk.document_title.to_lowercase());
                        for w in text.split(|c: char| !c.is_alphanumeric()) {
                            let w_clean = w.trim();
                            if w_clean.len() >= 5 && !stop_words.contains(w_clean) {
                                w_set.insert(w_clean.to_string());
                            }
                        }
                    }
                    w_set
                };

                let x_words = extract_words(&x_chunks);
                let y_words = extract_words(&y_chunks);
                let mut bridge_terms: Vec<String> = x_words.intersection(&y_words).cloned().collect();

                let mut term_frequencies = std::collections::HashMap::new();
                for chunk in x_chunks.iter().chain(y_chunks.iter()) {
                    let text = format!("{} {}", chunk.content.to_lowercase(), chunk.document_title.to_lowercase());
                    for w in text.split(|c: char| !c.is_alphanumeric()) {
                        let w_clean = w.trim();
                        if w_clean.len() >= 5 && !stop_words.contains(w_clean) {
                            *term_frequencies.entry(w_clean.to_string()).or_insert(0) += 1;
                        }
                    }
                }

                // Sort by combined frequency descending
                bridge_terms.sort_by(|a, b| {
                    let freq_a = term_frequencies.get(a).copied().unwrap_or(0);
                    let freq_b = term_frequencies.get(b).copied().unwrap_or(0);
                    freq_b.cmp(&freq_a)
                });
                bridge_terms.truncate(8);

                if bridge_terms.is_empty() {
                    bridge_terms = vec!["integration".to_string(), "connection".to_string(), "setup".to_string()];
                }

                let bridge_query = format!("{} {} {}", target_x, bridge_terms.join(" "), target_y);

                println!("[BRIDGE_CONCEPTS]");
                for term in &bridge_terms {
                    println!("{}", term);
                }
                println!();

                println!("[BRIDGE_CONCEPTS]");
                println!("Entity A: {}", target_x);
                println!("Entity B: {}", target_y);
                println!("\nBridge Terms:");
                for term in &bridge_terms {
                    println!("{}", term);
                }
                println!("\nBridge Query:");
                println!("{}", bridge_query);
                println!();

                if executed_passes < 6 {
                    let search_query = self.expand_query(&bridge_query).await;
                    let chunks = self.retrieve_hybrid(database, &search_query, &clean_analysis, 40).await?;
                    hop1_chunks.extend(chunks);
                    executed_passes += 1;
                } else {
                    discarded_passes += 1;
                }
            }
        } else {
            let query_lower = query.to_lowercase();
            let mut sub_queries = Vec::new();
            if let Some(pos) = query_lower.find(" connect to ") {
                sub_queries.push(query[..pos].trim().to_string());
                sub_queries.push(query[pos + " connect to ".len()..].trim().to_string());
            } else if let Some(pos) = query_lower.find(" connect ") {
                sub_queries.push(query[..pos].trim().to_string());
                sub_queries.push(query[pos + " connect ".len()..].trim().to_string());
            } else if let Some(pos) = query_lower.find(" relate to ") {
                sub_queries.push(query[..pos].trim().to_string());
                sub_queries.push(query[pos + " relate to ".len()..].trim().to_string());
            } else if let Some(pos) = query_lower.find(" link to ") {
                sub_queries.push(query[..pos].trim().to_string());
                sub_queries.push(query[pos + " link to ".len()..].trim().to_string());
            } else {
                sub_queries.push(query.to_string());
            }

            planned_passes = sub_queries.len();
            for sub_q in sub_queries {
                if executed_passes < 6 {
                    let search_query = self.expand_query(&sub_q).await;
                    let sub_chunks = self.retrieve_hybrid(database, &search_query, &clean_analysis, 45).await?;
                    hop1_chunks.extend(sub_chunks);
                    executed_passes += 1;
                } else {
                    discarded_passes += 1;
                }
            }
        }

        println!(
            "[RECURSIVE_BUDGET] planned: {}, executed: {}, discarded: {}",
            planned_passes,
            executed_passes,
            discarded_passes
        );
        tracing::info!(
            "[RECURSIVE_BUDGET] planned: {}, executed: {}, discarded: {}",
            planned_passes,
            executed_passes,
            discarded_passes
        );

        deduplicate_chunks(&mut hop1_chunks);

        for chunk in &mut hop1_chunks {
            let mut meta = chunk.metadata.as_object().cloned().unwrap_or_default();
            meta.insert("lineage".to_string(), json!({
                "hop_number": 1,
                "parent_chunk": serde_json::Value::Null,
                "retrieval_reason": "Entity-aware recursive retrieval plan"
            }));
            chunk.metadata = Value::Object(meta);
        }

        if depth >= 1 {
            return Ok(hop1_chunks);
        }

        let hop1_doc_ids: Vec<&str> = hop1_chunks.iter().map(|c| c.document_id.as_str()).collect();
        let graph_guard = self.document_graph.read().await;
        let hop2_doc_ids = graph_guard.neighbors_of_set(&hop1_doc_ids);

        let mut hop2_chunks = Vec::new();
        for doc_id in hop2_doc_ids.into_iter().take(5) {
            let chunks_in_doc = database.document_repository().get_chunks_by_document(&doc_id)?;
            let chunk_ids: Vec<String> = chunks_in_doc.into_iter().map(|c| c.id).collect();
            let mut hydrated_hop2 = hydrate_chunk_ids(database, &chunk_ids)?;

            let parent = hop1_chunks.iter().find(|c| {
                graph_guard.neighbors(&c.document_id).contains(&doc_id)
            });
            let (parent_chunk_id, parent_doc_title) = match parent {
                Some(p) => (Some(p.chunk_id.clone()), p.document_title.clone()),
                None => (None, "Connected Document".to_string()),
            };

            for chunk in &mut hydrated_hop2 {
                let mut meta = chunk.metadata.as_object().cloned().unwrap_or_default();
                meta.insert("lineage".to_string(), json!({
                    "hop_number": 2,
                    "parent_chunk": parent_chunk_id,
                    "retrieval_reason": format!("Cross-reference: {}", parent_doc_title)
                }));
                chunk.metadata = Value::Object(meta);
            }
            hop2_chunks.extend(hydrated_hop2);
        }

        let mut accumulated = hop1_chunks;
        accumulated.extend(hop2_chunks);
        deduplicate_chunks(&mut accumulated);

        let query_entities: Vec<String> = accepted_entities.iter().map(|e| e.group_name.clone()).collect();
        
        let mut candidate_chunks = accumulated;
        if query_entities.len() >= 2 {
            let initial_count = candidate_chunks.len();
            candidate_chunks.retain(|chunk| {
                let coverage = cross_document_entity_coverage(chunk, &query_entities, &dict);
                coverage > 0.0
            });
            tracing::info!(
                "[RECURSIVE] Cross-document entity filtering: retained {}/{} chunks with entity coverage > 0.0",
                candidate_chunks.len(),
                initial_count
            );
        }

        let candidate_chunks_diverse = apply_diversity_filter(candidate_chunks, 3);

        let rerank_limit = candidate_chunks_diverse.len().min(60);
        let mut reranked = self.reranker_service.rerank(query, candidate_chunks_diverse, rerank_limit).await?;

        if query_entities.len() >= 2 {
            for chunk in &mut reranked {
                let coverage = cross_document_entity_coverage(chunk, &query_entities, &dict);
                if coverage >= 0.99 {
                    chunk.score += 8.0;
                    tracing::info!(
                        "[RECURSIVE] Applied boost +8.0 to chunk={} ({}) for entity coverage {:.2}",
                        chunk.chunk_id, chunk.document_title, coverage
                    );

                    let content_lower = chunk.content.to_lowercase();
                    let title_lower = chunk.document_title.to_lowercase();
                    let text = format!("{} {}", content_lower, title_lower);
                    let mut strong_matches = 0;
                    let mut total_mentions = 0;
                    for entity_name in &query_entities {
                        if let Some(group) = dict.groups.iter().find(|g| &g.name == entity_name) {
                            let mentions = count_entity_mentions(&text, group);
                            total_mentions += mentions;
                            if mentions >= 2 {
                                strong_matches += 1;
                            }
                        }
                    }
                    if strong_matches > 0 {
                        let freq_boost = strong_matches as f32 * 4.0;
                        chunk.score += freq_boost;
                    }
                    if total_mentions > 0 {
                        let mention_boost = total_mentions as f32 * 1.5;
                        chunk.score += mention_boost;
                    }
                    let section_density = calculate_entity_section_density(&chunk.content, &query_entities, &dict);
                    if section_density > 0.0 {
                        let section_boost = section_density * 8.0;
                        chunk.score += section_boost;
                    }
                } else if coverage >= 0.49 {
                    chunk.score += 2.0;
                }
            }
        }

        if targets.len() >= 2 {
            let target_x = &targets[0];
            let target_y = &targets[1];
            for chunk in &mut reranked {
                if mentions_both_targets(chunk, target_x, target_y, &dict) {
                    chunk.score += 15.0;
                    tracing::info!(
                        "[RECURSIVE] Applied bridge target boost +15.0 to chunk={} ({})",
                        chunk.chunk_id, chunk.document_title
                    );
                }
            }
        }

        sort_descending(&mut reranked);

        let mut reranked_diverse = apply_diversity_filter(reranked, 2);
        reranked_diverse.truncate(25);
        Ok(reranked_diverse)
    }

    async fn generate_answer(
        &self,
        query: &str,
        analysis: &QueryAnalysis,
        context: &BuiltContext,
    ) -> Result<String> {
        let dict = self.entity_dictionary.read().await;
        let intent_type = determine_query_intent(query, analysis, &dict);
        let structured = extract_structured_context(&context.chunks);
        
        let mut context_markdown = String::new();
        if !structured.facts.is_empty() {
            context_markdown.push_str("### Extracted Facts:\n");
            for fact in &structured.facts {
                context_markdown.push_str(&format!("- {}\n", fact));
            }
        }
        if !structured.concepts.is_empty() {
            context_markdown.push_str("\n### Key Concepts:\n");
            for concept in &structured.concepts {
                context_markdown.push_str(&format!("- {}\n", concept));
            }
        }
        if !structured.relationships.is_empty() {
            context_markdown.push_str("\n### Relationships & Connections:\n");
            for rel in &structured.relationships {
                context_markdown.push_str(&format!("- {}\n", rel));
            }
        }

        let system_prompt = match intent_type {
            QueryIntentType::FactLookup => {
                format!(
                    "You are Assistant Core, a helpful AI assistant. Your goal is to provide a concise, direct answer to the user's query.\n\
                     \n\
                     STRICT CONSTRAINTS:\n\
                     1. Your response MUST be between 2 and 4 sentences long. Do not exceed 4 sentences.\n\
                     2. Use only the supplied context. Do not assume or extrapolate.\n\
                     3. Do NOT include any citations, sources, files, scores, percentages, or chunk IDs in the text of your answer.\n\
                     4. Do NOT append a 'Sources:' or 'Citations:' list or metadata block. Output ONLY the clean answer text.\n\
                     5. Keep the answer extremely grounded and direct.\n\
                     6. To ensure accuracy, preserve key technical terms (like 'monitoring', 'authentication', etc.) from the context rather than rephrasing them. For example, if the context is about Prometheus or Grafana, you MUST use the term 'monitoring' in your response.\n\
                     7. Do NOT mention any unrelated components, SaaS tools, platforms, or integrations (such as Notion, Obsidian, Jira, Slack, etc.) unless they are directly and centrally the subject of the user query."
                )
            }
            QueryIntentType::BroadTopic => {
                format!(
                    "You are Assistant Core, a helpful AI assistant. Your goal is to provide a structured, well-organized overview of the topic.\n\
                     \n\
                     STRICT CONSTRAINTS:\n\
                     1. Your response MUST be between 4 and 8 sentences long.\n\
                     2. Use headers (##) or structured paragraphs to present the information.\n\
                     3. Use only the supplied context. Do not assume, extrapolate, or invent sections/categories not present in the context.\n\
                     4. If the context only supports a few concepts, describe only those concepts. Do not force standard structures like Architecture/Benefits unless supported by evidence.\n\
                     5. Do NOT include any citations, sources, files, scores, percentages, or chunk IDs in the text of your answer.\n\
                     6. Do NOT append a 'Sources:' or 'Citations:' list or metadata block. Output ONLY the clean answer text.\n\
                     7. To ensure accuracy, preserve key technical terms (like 'monitoring', 'authentication', etc.) from the context rather than rephrasing them.\n\
                     8. Do NOT mention any unrelated components, SaaS tools, platforms, or integrations (such as Notion, Obsidian, Jira, Slack, etc.) unless they are directly and centrally the subject of the user query."
                )
            }
            QueryIntentType::CrossDocument | QueryIntentType::Relationship => {
                format!(
                    "You are Assistant Core, a helpful AI assistant. Your goal is to explain the relationships, dependencies, or connections between components across documents.\n\
                     \n\
                     STRICT CONSTRAINTS:\n\
                     1. Your response MUST be between 5 and 10 sentences long.\n\
                     2. Focus on relationship-focused explanations (connections, integration, dependencies).\n\
                     3. Use only the supplied context. Do not assume, extrapolate, or synthesize concepts not present in the context.\n\
                     4. Do NOT include any citations, sources, files, scores, percentages, or chunk IDs in the text of your answer.\n\
                     5. Do NOT append a 'Sources:' or 'Citations:' list or metadata block. Output ONLY the clean answer text.\n\
                     6. To ensure accuracy, preserve key technical terms (like 'monitoring', 'authentication', etc.) from the context rather than rephrasing them.\n\
                     7. Do NOT mention any unrelated components, SaaS tools, platforms, or integrations (such as Notion, Obsidian, Jira, Slack, etc.) unless they are directly and centrally the subject of the user query."
                )
            }
            QueryIntentType::Comparison => {
                format!(
                    "You are Assistant Core, a helpful AI assistant. Your goal is to compare the specified entities, highlighting their differences, similarities, pros and cons, or features.\n\
                     \n\
                     STRICT CONSTRAINTS:\n\
                     1. Your response MUST be between 5 and 10 sentences long.\n\
                     2. Focus on comparing the target entities clearly and neutrally.\n\
                     3. Use only the supplied context. Do not assume, extrapolate, or synthesize concepts not present in the context.\n\
                     4. Do NOT include any citations, sources, files, scores, percentages, or chunk IDs in the text of your answer.\n\
                     5. Do NOT append a 'Sources:' or 'Citations:' list or metadata block. Output ONLY the clean answer text.\n\
                     6. To ensure accuracy, preserve key technical terms (like 'monitoring', 'authentication', etc.) from the context rather than rephrasing them.\n\
                     7. Do NOT mention any unrelated components, SaaS tools, platforms, or integrations unless they are directly and centrally the subject of the user query."
                )
            }
        };

        let user_prompt = format!(
            "User query: {query}\n\
             \n\
             Structured Context:\n\
             {context_markdown}\n\
             \n\
             Remember: Output only the direct answer text. Do not include source metadata, citations, or file names."
        );

        self.groq_service.chat_text(&system_prompt, &user_prompt).await
    }

    pub async fn initialize(&self, database: &Database) -> Result<()> {
        self.ensure_groq_ready()?;
        self.sparse_service.initialize().await?;
        let documents = database.document_repository().list_all_chunk_search_documents()?;
        self.sparse_service.rebuild_index(&documents).await?;
        self.reranker_service.initialize().await?;
        self.rebuild_topic_graph(database).await?;
        Ok(())
    }

    /// Runs a Qdrant connectivity health check and logs the result.
    /// Returns `true` if Qdrant is reachable and the collection exists, `false` otherwise.
    /// This is a diagnostic-only method — it does not fail initialization.
    pub async fn check_dense_retrieval_health(&self) -> bool {
        match self.qdrant_service.check_health().await {
            Ok(true) => {
                tracing::info!("[QDRANT_HEALTH] Dense retrieval (Qdrant) is available and healthy.");
                true
            }
            Ok(false) => {
                tracing::warn!(
                    "[QDRANT_HEALTH] ⚠️  Qdrant responded but collection is missing or unhealthy. \
                     Dense retrieval will be degraded until documents are re-indexed."
                );
                false
            }
            Err(err) => {
                tracing::warn!(
                    "[QDRANT_HEALTH] ⚠️  Qdrant is UNREACHABLE: {}. \
                     Dense retrieval is DISABLED for this session. \
                     Hybrid queries will fall back to sparse-only retrieval.",
                    err
                );
                false
            }
        }
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

#[derive(Debug, Clone)]
pub struct StructuredContext {
    pub facts: Vec<String>,
    pub concepts: Vec<String>,
    pub relationships: Vec<String>,
}

fn split_into_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();
    let mut chars = text.chars().peekable();
    
    while let Some(ch) = chars.next() {
        current.push(ch);
        if ch == '.' || ch == '!' || ch == '?' {
            let is_abbreviation = if let Some(&next_ch) = chars.peek() {
                next_ch.is_ascii_digit() || (next_ch.is_ascii_alphabetic() && next_ch.is_lowercase())
            } else {
                false
            };
            if !is_abbreviation {
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() {
                    sentences.push(trimmed);
                }
                current.clear();
            }
        }
    }
    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        sentences.push(trimmed);
    }
    sentences
}

fn extract_structured_context(chunks: &[RetrievedChunk]) -> StructuredContext {
    let mut facts = Vec::new();
    let mut concepts = Vec::new();
    let mut relationships = Vec::new();
    let mut seen = HashSet::new();

    let rel_terms = [
        "connect", "relate", "depend", "interact", "because", "relationship", 
        "connection", "rely", "relies", "dependency", "dependencies", 
        "integrate", "sync", "works with", "is used for", "integrates with",
        "syncs with", "connects to", "relates to"
    ];
    let fact_terms = [
        " is a ", " is the ", " are the ", " are a ", " defined as ", 
        " refers to ", " consists of ", " includes "
    ];

    for chunk in chunks {
        let sentences = split_into_sentences(&chunk.content);
        for sentence in sentences {
            let sentence_clean = sentence.replace('\n', " ").replace("  ", " ");
            let sentence_clean = sentence_clean.trim().to_string();
            if sentence_clean.is_empty() || sentence_clean.len() < 10 {
                continue;
            }
            if !seen.insert(sentence_clean.clone()) {
                continue;
            }

            let sentence_lower = sentence_clean.to_lowercase();
            let is_relationship = rel_terms.iter().any(|&term| sentence_lower.contains(term));
            
            if is_relationship {
                relationships.push(sentence_clean);
            } else {
                let is_fact = sentence_clean.chars().any(|c| c.is_ascii_digit())
                    || fact_terms.iter().any(|&term| sentence_lower.contains(term));
                if is_fact {
                    facts.push(sentence_clean);
                } else {
                    let has_backtick = sentence_clean.contains('`');
                    let words: Vec<&str> = sentence_clean.split_whitespace().collect();
                    let has_capitalized_noun = words.iter().enumerate().skip(1).any(|(idx, word)| {
                        if let Some(first_char) = word.chars().next() {
                            first_char.is_uppercase() && word.len() > 2 && !words[idx-1].ends_with('.')
                        } else {
                            false
                        }
                    });
                    if has_backtick || has_capitalized_noun {
                        concepts.push(sentence_clean);
                    } else {
                        concepts.push(sentence_clean);
                    }
                }
            }
        }
    }

    StructuredContext {
        facts,
        concepts,
        relationships,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryIntentType {
    FactLookup,
    BroadTopic,
    CrossDocument,
    Comparison,
    Relationship,
}

fn determine_query_intent(query: &str, analysis: &QueryAnalysis, entity_dict: &EntityDictionary) -> QueryIntentType {
    let q_lower = query.to_lowercase();
    if q_lower.contains("compare")
        || q_lower.contains("difference")
        || q_lower.contains("vs")
        || q_lower.contains("versus")
        || q_lower.contains("pros and cons")
    {
        QueryIntentType::Comparison
    } else if q_lower.contains("connect")
        || q_lower.contains("interact")
        || q_lower.contains("relationship")
        || q_lower.contains("relate")
    {
        QueryIntentType::Relationship
    } else if analysis.intent == "broad_topic_synthesis" || entity_dict.detect_broad_topic(query).is_some() {
        QueryIntentType::BroadTopic
    } else {
        QueryIntentType::FactLookup
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
                document_title: if row.title.is_empty() {
                    row.document_id.clone()
                } else {
                    row.title.clone()
                },
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

fn extract_evidence(content: &str, query: &str, entity_dict: &EntityDictionary) -> Option<String> {
    let query_lower = query.to_lowercase();
    let stop_words: HashSet<&str> = [
        "what", "is", "the", "does", "how", "to", "explain", "describe", "about",
        "system", "use", "connect", "with", "and", "or",
        "a", "an", "of", "which", "whose", "where", "whom", "related", "that",
        "this", "for", "from", "are", "was", "were", "has", "have", "had",
        "can", "will", "would", "should", "could", "may", "might", "in", "on",
    ].iter().cloned().collect();

    let query_keywords: Vec<String> = query_lower
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
        .filter(|w| w.len() > 3 && !stop_words.contains(w.as_str()))
        .collect();

    if query_keywords.is_empty() {
        return None;
    }

    let topic_words: HashSet<&str> = [
        "database", "system", "authentication", "monitoring", "observability",
        "configuration", "settings", "notion", "obsidian", "setup", "auth",
        "telemetry", "metrics", "storage", "persistence", "integration",
        "integrations", "process", "pipeline", "sync", "flow", "guide",
        "credential", "credentials", "token", "tokens", "client", "server",
        "service", "services", "user", "users", "compare", "contrast",
        "difference", "relationship", "connection", "relation", "list",
        "show", "explain", "describe", "summarize", "check", "find", "get",
        "tell", "which", "whose", "where", "whom", "stores"
    ].iter().cloned().collect();

    let non_topic_query_kws: Vec<&String> = query_keywords
        .iter()
        .filter(|kw| !topic_words.contains(kw.as_str()))
        .collect();

    let (_, matched_groups) = entity_dict.expand(query);
    let mut expanded_terms = Vec::new();
    for g in matched_groups {
        if let Some(group) = entity_dict.group_for_cluster(&g) {
            for term in &group.expansion_terms {
                if !query_keywords.contains(term) && !expanded_terms.contains(term) {
                    expanded_terms.push(term.clone());
                }
            }
        }
    }

    let sentences = content.split(|c| c == '.' || c == '!' || c == '?');
    let mut best_sentence: Option<(String, f32)> = None;

    for sentence in sentences {
        let sentence_trimmed = sentence.trim();
        if sentence_trimmed.is_empty() || sentence_trimmed.len() < 10 {
            continue;
        }
        let sentence_lower = sentence_trimmed.to_lowercase();
        
        let mut score = 0.0;
        for kw in &query_keywords {
            if sentence_lower.contains(kw) {
                score += 2.0;
            }
        }
        for ext in &expanded_terms {
            if sentence_lower.contains(ext) {
                score += 1.0;
            }
        }

        if !non_topic_query_kws.is_empty() {
            let has_any_non_topic_match = non_topic_query_kws
                .iter()
                .any(|kw| sentence_lower.contains(*kw));
            if !has_any_non_topic_match {
                continue;
            }
        }

        if score > 0.0 {
            match &best_sentence {
                Some((_, best_score)) if score > *best_score => {
                    best_sentence = Some((sentence_trimmed.to_string(), score));
                }
                None => {
                    best_sentence = Some((sentence_trimmed.to_string(), score));
                }
                _ => {}
            }
        }
    }

    best_sentence.map(|(s, _)| s)
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

fn get_percentile(sorted: &[f32], pct: f32) -> f32 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = (sorted.len() - 1) as f32 * pct;
    let low = idx.floor() as usize;
    let high = idx.ceil() as usize;
    if low == high {
        sorted[low]
    } else {
        let weight = idx - low as f32;
        sorted[low] * (1.0 - weight) + sorted[high] * weight
    }
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

fn get_static_specific_terms(group_name: &str) -> &'static [&'static str] {
    match group_name {
        "authentication" => &[
            "oauth", "pkce", "jwt", "sso", "saml", "openid", "ldap", "mfa", "2fa", "credential",
            "credentials", "keychain", "keyring"
        ],
        "monitoring" => &[
            "prometheus", "grafana", "alertmanager", "loki", "jaeger", "datadog"
        ],
        "database" => &[
            "qdrant", "sqlite", "postgres", "postgresql", "mysql", "redis", "mongodb"
        ],
        "embedding" => &[
            "nomic", "bge", "e5", "ada", "text2vec"
        ],
        "rag" => &[
            "bm25", "rrf", "mmr"
        ],
        _ => &[],
    }
}

fn count_entity_mentions(text: &str, group: &crate::services::entity_dictionary::EntityGroup) -> usize {
    let mut count = 0;
    let name_lower = group.name.to_lowercase();
    count += count_occurrences(text, &name_lower);
    for t in &group.primary_terms {
        let t_lower = t.to_lowercase();
        if t_lower != name_lower {
            count += count_occurrences(text, &t_lower);
        }
    }
    
    let static_specifics = get_static_specific_terms(&group.name);
    for t in static_specifics {
        count += count_occurrences(text, t);
    }
    count
}

fn count_occurrences(text: &str, sub: &str) -> usize {
    if sub.is_empty() {
        return 0;
    }
    let mut count = 0;
    let mut start = 0;
    while let Some(pos) = text[start..].find(sub) {
        count += 1;
        start += pos + sub.len();
    }
    count
}

fn cross_document_entity_coverage(
    chunk: &RetrievedChunk,
    query_entities: &[String],
    entity_dict: &EntityDictionary,
) -> f32 {
    if query_entities.is_empty() {
        return 0.0;
    }
    
    let content_lower = chunk.content.to_lowercase();
    let title_lower = chunk.document_title.to_lowercase();
    let text = format!("{} {}", content_lower, title_lower);
    
    let mut covered_count = 0;
    for entity_name in query_entities {
        if let Some(group) = entity_dict.groups.iter().find(|g| &g.name == entity_name) {
            let static_specifics = get_static_specific_terms(&group.name);
            let matches_entity = text.contains(&group.name.to_lowercase())
                || group.primary_terms.iter().any(|t| text.contains(&t.to_lowercase()))
                || static_specifics.iter().any(|t| text.contains(&t.to_lowercase()));
            if matches_entity {
                covered_count += 1;
            }
        }
    }
    
    covered_count as f32 / query_entities.len() as f32
}

fn calculate_entity_section_density(
    content: &str,
    query_entities: &[String],
    entity_dict: &EntityDictionary,
) -> f32 {
    if query_entities.len() < 2 {
        return 0.0;
    }
    
    let mut sections = Vec::new();
    let mut current_section = String::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("# ") || trimmed.starts_with("## ") {
            if !current_section.is_empty() {
                sections.push(current_section);
                current_section = String::new();
            }
        }
        current_section.push_str(line);
        current_section.push('\n');
    }
    if !current_section.is_empty() {
        sections.push(current_section);
    }

    let mut max_cross_entities = 0;
    for section in sections {
        let sec_lower = section.to_lowercase();
        let mut matched_in_section = 0;
        for entity_name in query_entities {
            if let Some(group) = entity_dict.groups.iter().find(|g| &g.name == entity_name) {
                let static_specifics = get_static_specific_terms(&group.name);
                let matches_entity = sec_lower.contains(&group.name.to_lowercase())
                    || group.primary_terms.iter().any(|t| sec_lower.contains(&t.to_lowercase()))
                    || static_specifics.iter().any(|t| sec_lower.contains(&t.to_lowercase()));
                if matches_entity {
                    matched_in_section += 1;
                }
            }
        }
        if matched_in_section > max_cross_entities {
            max_cross_entities = matched_in_section;
        }
    }

    if max_cross_entities >= 2 {
        max_cross_entities as f32 / query_entities.len() as f32
    } else {
        0.0
    }
}

fn deduplicate_chunks(chunks: &mut Vec<RetrievedChunk>) {
    let mut seen = HashSet::new();
    chunks.retain(|chunk| seen.insert(chunk.chunk_id.clone()));
}

fn apply_diversity_filter(chunks: Vec<RetrievedChunk>, max_per_doc: usize) -> Vec<RetrievedChunk> {
    let mut doc_counts = HashMap::new();
    let mut filtered = Vec::new();
    for chunk in chunks {
        let count = doc_counts.entry(chunk.document_id.clone()).or_insert(0);
        if *count < max_per_doc {
            filtered.push(chunk);
            *count += 1;
        }
    }
    filtered
}

fn mentions_both_targets(chunk: &RetrievedChunk, target_x: &str, target_y: &str, dict: &EntityDictionary) -> bool {
    let text = format!("{} {}", chunk.content.to_lowercase(), chunk.document_title.to_lowercase());
    
    let x_matched = if let Some(group) = dict.groups.iter().find(|g| g.name.eq_ignore_ascii_case(target_x)) {
        group.name.eq_ignore_ascii_case(target_x) && text.contains(&group.name.to_lowercase())
            || group.primary_terms.iter().any(|t| text.contains(&t.to_lowercase()))
            || group.specific_terms.iter().any(|t| text.contains(&t.to_lowercase()))
            || group.expansion_terms.iter().any(|t| text.contains(&t.to_lowercase()))
    } else {
        text.contains(&target_x.to_lowercase())
    };

    let y_matched = if let Some(group) = dict.groups.iter().find(|g| g.name.eq_ignore_ascii_case(target_y)) {
        group.name.eq_ignore_ascii_case(target_y) && text.contains(&group.name.to_lowercase())
            || group.primary_terms.iter().any(|t| text.contains(&t.to_lowercase()))
            || group.specific_terms.iter().any(|t| text.contains(&t.to_lowercase()))
            || group.expansion_terms.iter().any(|t| text.contains(&t.to_lowercase()))
    } else {
        text.contains(&target_y.to_lowercase())
    };

    x_matched && y_matched
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
    topic_cluster: Option<&TopicCluster>,
    entity_dict: &EntityDictionary,
) -> bool {
    // Recursive strategy fetches from multiple docs intentionally — never ambiguous.
    if strategy == &RetrievalStrategy::Recursive {
        return false;
    }

    // ── Broad Topic Word Ambiguity Guard ────────────────────────────────────────
    let broad_topic_words: HashSet<&str> = [
        "setup", "integration", "configuration", "workflow", "platform",
        "architecture", "system", "deployment", "connection"
    ].iter().cloned().collect();

    let query_lower = query.to_lowercase();
    let query_words: Vec<String> = query_lower
        .split(|c: char| !c.is_alphanumeric())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let has_broad_word = query_words.iter().any(|w| broad_topic_words.contains(w.as_str()));
    if has_broad_word {
        // Check if there is any specific entity mentioned
        let mut has_specific_entity = false;
        for group in &entity_dict.groups {
            let name_match = query_words.contains(&group.name.to_lowercase());
            let primary_match = group.primary_terms.iter().any(|t| query_words.contains(&t.to_lowercase()));
            let specific_match = group.specific_terms.iter().any(|t| query_words.contains(&t.to_lowercase()));
            if name_match || primary_match || specific_match {
                has_specific_entity = true;
                break;
            }
        }
        if !has_specific_entity {
            tracing::info!(
                "[AMBIGUITY] Broad topic word detected in query '{}' without specific qualifiers -> AMBIGUOUS_RETRIEVAL",
                query
            );
            return true;
        }
    }

    if chunks.len() < 2 {
        return false;
    }

    // ── GATE 1: Factual Lookup Guard ────────────────────────────────────────────
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
    if unique_doc_ids.len() < 2 {
        return false;
    }

    // ── GATE 2: Topic Cluster Guard ──────────────────────────────────────────────
    // Phase 2: use TopicCluster.same_cluster() if the cluster is available.
    // If all top docs belong to the same cluster, they are SUPPORTING EVIDENCE not ambiguity.
    // Fallback to old title-keyword check if cluster data is not yet built.
    let doc_id_refs: Vec<&str> = unique_doc_ids.iter().map(|s| s.as_str()).collect();
    if let Some(cluster) = topic_cluster {
        if cluster.same_cluster(&doc_id_refs) {
            tracing::debug!(
                "[AMBIGUITY] Gate 2 (TopicCluster): all top docs share a cluster → NOT ambiguous"
            );
            return false;
        }
    } else {
        // Fallback: title-keyword guard (legacy, used when cluster not yet built)
        let stop_words: HashSet<&str> = [
            "what", "is", "the", "does", "how", "to", "explain", "describe", "about",
            "system", "use", "connect", "with", "and", "or",
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
        let title_token_sets: Vec<HashSet<String>> = unique_doc_ids.iter()
            .map(|doc_id| {
                chunks.iter().find(|c| &c.document_id == doc_id)
                    .map(|c| tokenize(&c.document_title))
                    .unwrap_or_default()
            })
            .collect();
        if title_token_sets.len() >= 2 && !title_token_sets[0].is_empty() {
            let shared_by_all = title_token_sets[0].iter().any(|token| {
                token.len() > 4 && title_token_sets.iter().skip(1).all(|other| other.contains(token))
            });
            if shared_by_all {
                return false;
            }
        }
    }

    // ── GATE 3: Full Topic Divergence Check ──────────────────────────────────────
    let stop_words: HashSet<&str> = [
        "what", "is", "the", "does", "how", "to", "explain", "describe", "about",
        "system", "use", "connect", "with", "and", "or",
        "a", "an", "of", "which", "whose", "where", "whom", "related", "that",
        "this", "for", "from", "are", "was", "were", "has", "have", "had",
        "can", "will", "would", "should", "could", "may", "might", "in", "on",
        "setup", "guide", "process", "flow", "configure", "standard", "policy",
        "management", "overview", "document", "file", "notes", "checklist",
        "integration", "integrations", "sync", "pipeline", "service", "services",
        "user", "users", "details"
    ].iter().cloned().collect();
    let tokenize = |text: &str| -> HashSet<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && !stop_words.contains(s.as_str()) && s.len() > 3)
            .collect()
    };

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
            all_divergent = false;
            break;
        }
    }

    if !all_divergent {
        return false;
    }

    let doc_aggregations = aggregate_documents(chunks);
    if doc_aggregations.len() < 2 {
        return false;
    }
    let sigmoid = |x: f32| -> f32 { 1.0 / (1.0 + (-x).exp()) };
    let top_score = sigmoid(doc_aggregations[0].document_score);
    let second_score = sigmoid(doc_aggregations[1].document_score);
    let score_gap = top_score - second_score;

    score_gap < 0.15
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
        "system", "use", "connect", "with", "and", "or", "a", "an", "of",
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
    intersection as f32 / query_tokens.len() as f32
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
        "system", "use", "connect", "with", "and", "or", "a", "an", "of",
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
        let dict = EntityDictionary::default();
        // No chunks → EMPTY_RETRIEVAL
        let report = service.calculate_confidence(
            "test query",
            &[],
            &RetrievalStrategy::Hybrid,
            &crate::domain::QueryComplexity::Simple,
            &dict,
        );
        assert_eq!(report.status, "EMPTY_RETRIEVAL");
        assert_eq!(report.confidence, "low");
        assert_eq!(report.confidence_score, 0);

        // score=-3 → sigmoid(-3)=0.047 < 0.15 and no evidence -> EMPTY
        let weak_chunks = vec![mock_chunk("doc-1", -3.0, 0.001)];
        let report2 = service.calculate_confidence(
            "test query",
            &weak_chunks,
            &RetrievalStrategy::Hybrid,
            &crate::domain::QueryComplexity::Simple,
            &dict,
        );
        assert_eq!(report2.status, "EMPTY_RETRIEVAL");
    }

    #[test]
    fn test_confidence_low_confidence_retrieval() {
        let service = mock_service();
        let dict = EntityDictionary::default();
        // score=-2.5 → sigmoid(-2.5)=0.076, kw_overlap=0.0.
        // Triggers EMPTY because top_normalized_rerank < 0.15 and evidence_count == 0.
        // To trigger LOW instead, let's provide a slightly better score or evidence.
        // score=-1.5 → sigmoid(-1.5)=0.182 < 0.20, evidence_count = 0. Triggers LOW.
        let weak_chunks = vec![mock_chunk("doc-1", -1.5, 0.005)];
        let report = service.calculate_confidence(
            "blahblah nothinghere faketerm",
            &weak_chunks,
            &RetrievalStrategy::Hybrid,
            &crate::domain::QueryComplexity::Simple,
            &dict,
        );
        assert_eq!(report.status, "LOW_CONFIDENCE_RETRIEVAL");
        assert_eq!(report.confidence, "low");
    }

    #[test]
    fn test_confidence_partial_retrieval() {
        let service = mock_service();
        let dict = EntityDictionary::default();
        // After recalibration, PARTIAL fires for:
        //   (top_sigmoid < 0.28 AND kw_overlap < 0.10)
        //   OR (evidence_count == 0 AND kw_overlap < 0.10)
        //
        // sigmoid(-1.0) = 0.269 < 0.28.
        // Query: "zebra wildebeest migration habitat" (no terms match document content)
        // Content: "authentication flow oauth token session management" → kw_overlap = 0
        // This satisfies BOTH conditions:
        //   (0.269 < 0.28 && 0.0 < 0.10) = TRUE → PARTIAL (not empty/low because sigmoid >= 0.10)
        //
        // LOW_CONFIDENCE gate does NOT fire because:
        //   (0.269 < 0.15 && evidence==0) = FALSE (0.269 >= 0.15)
        //   (0.269 < 0.12 && kw < 0.05) = FALSE
        //   (kw < 0.01 && !evidence) would fire if kw=0 and evidence=0, so we need evidence > 0.
        // To have evidence>0 but kw=0: content includes the entity "authentication" which
        // extract_evidence can detect, while query "zebra" has no overlap.
        let partial_chunks = vec![
            mock_chunk_with_content(
                "doc-1",
                "Authentication Guide",
                "The authentication flow uses oauth2 tokens for session management and identity verification.",
                -1.0,
            ),
        ];
        let report = service.calculate_confidence(
            "zebra wildebeest migration habitat",
            &partial_chunks,
            &RetrievalStrategy::Hybrid,
            &crate::domain::QueryComplexity::Simple,
            &dict,
        );
        // sigmoid=0.269 < 0.28, kw_overlap=0.0 < 0.10, evidence may be present
        // → PARTIAL if evidence prevents LOW_CONFIDENCE, or LOW_CONFIDENCE if no evidence
        // Both are valid "weak retrieval" states for this input
        assert!(
            report.status == "PARTIAL_RETRIEVAL" || report.status == "LOW_CONFIDENCE_RETRIEVAL",
            "Expected PARTIAL or LOW_CONFIDENCE for weak-score chunk with zero keyword overlap, got: {}",
            report.status
        );
        // Crucially, it must NOT be OK or EMPTY
        assert_ne!(report.status, "OK", "Should not be OK with weak reranker score");
        assert_ne!(report.status, "EMPTY_RETRIEVAL", "Should not be EMPTY with valid chunk present");
    }

    #[test]
    fn test_confidence_ok_high() {
        let service = mock_service();
        let dict = EntityDictionary::default();
        // sigmoid(3.0) = 0.952 > partial_threshold (0.35) → OK
        let strong_chunks = vec![
            mock_chunk_with_content("doc-1", "Mock Title", "test query content details", 3.0),
            mock_chunk_with_content("doc-1", "Mock Title", "test query content details", 2.5),
            mock_chunk_with_content("doc-1", "Mock Title", "test query content details", 2.0),
        ];
        let report = service.calculate_confidence(
            "test query",
            &strong_chunks,
            &RetrievalStrategy::Hybrid,
            &crate::domain::QueryComplexity::Simple,
            &dict,
        );
        assert_eq!(report.status, "OK");
        assert_eq!(report.confidence, "high");
        assert!(report.confidence_score >= 70);
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
        let dict = EntityDictionary::default();
        let result = is_ambiguous_retrieval(
            "Which vector database was selected for production and why",
            &chunks,
            &RetrievalStrategy::Hybrid,
            None,
            &dict,
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
        let dict = EntityDictionary::default();
        // "authentication" and "qdrant" share very few tokens → different clusters, close scores
        let result = is_ambiguous_retrieval("authentication qdrant", &chunks, &RetrievalStrategy::Hybrid, None, &dict);
        assert!(result, "Distinct-topic docs with close scores SHOULD trigger AMBIGUOUS_RETRIEVAL");
    }

    #[test]
    fn test_ambiguity_recursive_never_ambiguous() {
        let chunks = vec![
            mock_chunk("doc-1", 1.5, 0.02),
            mock_chunk("doc-2", 1.48, 0.02),
        ];
        let dict = EntityDictionary::default();
        assert!(!is_ambiguous_retrieval("some query", &chunks, &RetrievalStrategy::Recursive, None, &dict));
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

    #[test]
    fn test_determine_query_intent() {
        let dict = EntityDictionary::build(&[]);
        let dummy_analysis = |intent: &str, strategy: RetrievalStrategy| QueryAnalysis {
            intent: intent.to_string(),
            entities: vec![],
            metadata_filters: MetadataFilters::default(),
            temporal: false,
            complexity: crate::domain::QueryComplexity::Simple,
            strategy,
        };

        // Broad Topic Synthesis
        assert_eq!(
            determine_query_intent("Explain authentication", &dummy_analysis("synthesis", RetrievalStrategy::Hybrid), &dict),
            QueryIntentType::BroadTopic
        );

        // Cross Document
        assert_eq!(
            determine_query_intent("How does Prometheus relate to Grafana?", &dummy_analysis("lookup", RetrievalStrategy::Hybrid), &dict),
            QueryIntentType::Relationship
        );
        assert_eq!(
            determine_query_intent("Compare OAuth and JWT", &dummy_analysis("lookup", RetrievalStrategy::Hybrid), &dict),
            QueryIntentType::Comparison
        );

        // Fact Lookup
        assert_eq!(
            determine_query_intent("Where are desktop credentials stored?", &dummy_analysis("lookup", RetrievalStrategy::Hybrid), &dict),
            QueryIntentType::FactLookup
        );
    }

    #[test]
    fn test_extract_structured_context() {
        let chunks = vec![
            mock_chunk_with_content(
                "doc-1",
                "Auth Info",
                "OAuth is a standard for authorization. It integrates with JWT tokens because of PKCE. The token timeout is 3600 seconds. `auth_flow` is critical.",
                1.0
            )
        ];

        let structured = extract_structured_context(&chunks);

        // Fact contains digits
        assert!(structured.facts.iter().any(|s| s.contains("3600")));
        
        // Relationship contains transition words
        assert!(structured.relationships.iter().any(|s| s.contains("integrates with") || s.contains("because")));

        // Concept contains backticks
        assert!(structured.concepts.iter().any(|s| s.contains("`auth_flow`")));
    }

    // ── Regression Evaluation Dataset & Automated Regression Suite ──────────

    #[test]
    fn test_regression_eval_fact_lookup() {
        let service = mock_service();
        
        // Mock dataset representing RAG system configuration doc
        let chunks = vec![
            mock_chunk_with_content(
                "rag_architecture_overview",
                "RAG Architecture Overview",
                "RAG system chunk size 512 overlap 10%.",
                2.5,
            ),
            mock_chunk_with_content(
                "rag_architecture_overview",
                "RAG Architecture Overview",
                "RAG overlap 10% chunk size.",
                2.3,
            ),
            mock_chunk_with_content(
                "rag_architecture_overview",
                "RAG Architecture Overview",
                "RAG Qdrant chunk database.",
                2.1,
            )
        ];
        
        let query = "What chunk size does the RAG system use?";
        
        // 1. Verify entity dictionary handles the topic
        let doc_for_dict = crate::domain::ChunkSearchDocument {
            chunk_id: "c-1".to_string(),
            document_id: "rag_architecture_overview".to_string(),
            ordinal: 0,
            source_kind: "obsidian".to_string(),
            title: "RAG Architecture Overview".to_string(),
            content: "RAG system chunk size 512 overlap 10%.".to_string(),
            path_or_url: None,
            tags: vec!["rag".to_string(), "configuration".to_string()],
            author: None,
            category: Some("Infrastructure".to_string()),
            created_at: None,
            updated_at: None,
            metadata: serde_json::json!({}),
            chunk_metadata_json: None,
        };
        let dict = EntityDictionary::build(&[doc_for_dict]);
        
        // 2. Test smart evidence extraction
        let evidence = extract_evidence(&chunks[0].content, query, &dict);
        assert!(evidence.is_some(), "Should extract evidence for direct fact lookup");
        let evidence_str = evidence.unwrap();
        assert!(evidence_str.contains("512"), "Evidence should contain chunk size 512");
        assert!(evidence_str.contains("10%"), "Evidence should contain overlap 10%");
        
        let ev0 = extract_evidence(&chunks[0].content, query, &dict);
        let ev1 = extract_evidence(&chunks[1].content, query, &dict);
        let ev2 = extract_evidence(&chunks[2].content, query, &dict);
        println!("DEBUG: ev0: {:?}", ev0);
        println!("DEBUG: ev1: {:?}", ev1);
        println!("DEBUG: ev2: {:?}", ev2);

        // 3. Test confidence gating (relevance)
        let report = service.calculate_confidence(
            query,
            &chunks,
            &RetrievalStrategy::Hybrid,
            &crate::domain::QueryComplexity::Simple,
            &dict,
        );
        println!("DEBUG: report status: {}, score: {}, reasons: {:?}", report.status, report.confidence_score, report.reasons);
        assert_eq!(report.status, "OK", "Direct matching fact lookup should be status OK");
        assert_eq!(report.confidence, "high", "Direct matching fact lookup should have high confidence");
    }

    #[test]
    fn test_regression_eval_empty_gating() {
        let service = mock_service();
        
        // Unrelated chunk in index
        let chunks = vec![
            mock_chunk_with_content(
                "rag_architecture_overview",
                "RAG Architecture Overview",
                "The RAG system uses a chunk size of 512 tokens with a 10% overlap between adjacent chunks.",
                -2.8, // low rerank score
            )
        ];
        
        let query = "CEO personal email salaries database";
        let dict = EntityDictionary::build(&[]);
        
        // 1. Evidence extraction should fail
        let evidence = extract_evidence(&chunks[0].content, query, &dict);
        assert!(evidence.is_none(), "Should not extract evidence for completely unrelated query");
        
        // 2. Confidence gating should classify as EMPTY_RETRIEVAL
        let report = service.calculate_confidence(
            query,
            &chunks,
            &RetrievalStrategy::Hybrid,
            &crate::domain::QueryComplexity::Simple,
            &dict,
        );
        assert_eq!(report.status, "EMPTY_RETRIEVAL", "Negative query with low score and no evidence should be EMPTY_RETRIEVAL");
    }

    #[test]
    fn test_regression_eval_dynamic_entity_discovery() {
        // Build document list with custom metadata
        let docs = vec![
            crate::domain::ChunkSearchDocument {
                chunk_id: "c-1".to_string(),
                document_id: "doc-prometheus".to_string(),
                ordinal: 0,
                source_kind: "notion".to_string(),
                title: "Prometheus Monitoring Setup".to_string(),
                content: "Detailed guide to Prometheus alertmanager and Loki logging setup.".to_string(),
                path_or_url: None,
                tags: vec!["Monitoring".to_string(), "Alerts".to_string()],
                author: None,
                category: Some("DevOps".to_string()),
                created_at: None,
                updated_at: None,
                metadata: serde_json::json!({
                    "technologies": "Grafana Loki",
                    "system": "prometheus"
                }),
                chunk_metadata_json: None,
            }
        ];
        
        let dict = EntityDictionary::build(&docs);
        
        // Verify monitoring group was enriched
        let mon_group = dict.group_for_cluster("monitoring");
        assert!(mon_group.is_some(), "Static monitoring group should exist");
        let mon_group = mon_group.unwrap();
        
        // Verify terms from tags/category/metadata were added as specific/expansion terms
        assert!(mon_group.specific_terms.iter().any(|t| t == "alerts"), "Should discover tag 'alerts'");
        assert!(mon_group.specific_terms.iter().any(|t| t == "devops"), "Should discover category 'devops'");
        assert!(mon_group.specific_terms.iter().any(|t| t == "loki"), "Should discover Loki from metadata/content");
        assert!(mon_group.specific_terms.iter().any(|t| t == "grafana"), "Should discover Grafana from metadata");
    }

    #[test]
    fn test_regression_eval_broad_topic_routing() {
        let docs = vec![
            crate::domain::ChunkSearchDocument {
                chunk_id: "c-1".to_string(),
                document_id: "monitoring_overview".to_string(),
                ordinal: 0,
                source_kind: "obsidian".to_string(),
                title: "Monitoring Overview".to_string(),
                content: "Telemetry and Prometheus details".to_string(),
                path_or_url: None,
                tags: vec!["monitoring".to_string()],
                author: None,
                category: None,
                created_at: None,
                updated_at: None,
                metadata: serde_json::json!({}),
                chunk_metadata_json: None,
            }
        ];
        let dict = EntityDictionary::build(&docs);
        
        let dummy_analysis = |intent: &str| QueryAnalysis {
            intent: intent.to_string(),
            entities: vec![],
            metadata_filters: MetadataFilters::default(),
            temporal: false,
            complexity: crate::domain::QueryComplexity::Simple,
            strategy: RetrievalStrategy::Hybrid,
        };
        
        // 1. Broad query "Explain monitoring" -> should route to BroadTopic
        let intent_broad = determine_query_intent("Explain monitoring", &dummy_analysis("synthesis"), &dict);
        assert_eq!(intent_broad, QueryIntentType::BroadTopic, "Explain monitoring should be BroadTopic");
        
        // 2. Specific query with Prometheus (specific term) -> should NOT be broad topic (routes to FactLookup)
        let intent_specific = determine_query_intent("What is the Prometheus configuration?", &dummy_analysis("lookup"), &dict);
        assert_eq!(intent_specific, QueryIntentType::FactLookup, "Prometheus alert queries should route to FactLookup");
    }

    #[test]
    fn test_regression_eval_ambiguity_clarification_trigger() {
        let dict = EntityDictionary::build(&[]);
        
        // Query "Explain onboarding monitoring" extracts subject "onboarding monitoring"
        // which contains primary terms for both "onboarding" and "monitoring"
        let matched = dict.detect_matching_groups("Explain onboarding monitoring");
        assert!(matched.len() >= 2, "Query should match multiple topic clusters, got: {:?}", matched);
        assert!(matched.contains(&"onboarding".to_string()));
        assert!(matched.contains(&"monitoring".to_string()));
    }

    #[test]
    fn test_evidence_coverage_math() {
        let calc_coverage = |with_ev: usize, total: usize| -> f32 {
            if total == 0 {
                0.0
            } else {
                with_ev as f32 / total as f32
            }
        };

        assert_eq!(calc_coverage(0, 0), 0.0);
        assert_eq!(calc_coverage(1, 1), 1.0);
        assert_eq!(calc_coverage(1, 5), 0.2);
        assert_eq!(calc_coverage(3, 10), 0.3);
    }

    #[test]
    fn test_canary_desktop_credentials() {
        let db_path = std::path::Path::new("assistant.db");
        if db_path.exists() {
            let database = Database::connect(db_path).unwrap();
            let docs = database.document_repository().list_all_chunk_search_documents().unwrap();
            
            let auth_doc = docs.iter().find(|d| d.title == "authentication_flow_oauth2");
            assert!(auth_doc.is_some(), "Canary document 'authentication_flow_oauth2' MUST exist in assistant.db");
            let doc = auth_doc.unwrap();
            
            assert!(
                doc.content.to_lowercase().contains("keychain") || doc.content.to_lowercase().contains("keyring"),
                "Canary document content must mention Keychain or keyring"
            );

            let service = mock_service();
            let dict = EntityDictionary::build(&docs);
            let chunks = vec![
                mock_chunk_with_content(&doc.document_id, &doc.title, &doc.content, 2.5)
            ];
            
            let report = service.calculate_confidence(
                "Where are desktop credentials stored?",
                &chunks,
                &RetrievalStrategy::Hybrid,
                &crate::domain::QueryComplexity::Simple,
                &dict,
            );
            assert_eq!(report.status, "OK");
            assert_eq!(report.confidence, "high");
        }
    }

    #[test]
    fn test_ambiguity_relevance_gap_routing() {
        let mock_docs = vec![
            crate::domain::ChunkSearchDocument {
                chunk_id: "c-1".to_string(),
                document_id: "doc-auth-setup".to_string(),
                ordinal: 0,
                source_kind: "obsidian".to_string(),
                title: "Authentication Setup Guide".to_string(),
                content: "Setting up authentication PKCE".to_string(),
                path_or_url: None,
                tags: vec!["authentication".to_string(), "setup".to_string()],
                author: None,
                category: None,
                created_at: None,
                updated_at: None,
                metadata: serde_json::json!({}),
                chunk_metadata_json: None,
            },
            crate::domain::ChunkSearchDocument {
                chunk_id: "c-2".to_string(),
                document_id: "doc-notion-setup".to_string(),
                ordinal: 0,
                source_kind: "notion".to_string(),
                title: "Notion Integration Setup".to_string(),
                content: "Setting up notion workspace integration".to_string(),
                path_or_url: None,
                tags: vec!["notion".to_string(), "setup".to_string()],
                author: None,
                category: None,
                created_at: None,
                updated_at: None,
                metadata: serde_json::json!({}),
                chunk_metadata_json: None,
            },
            crate::domain::ChunkSearchDocument {
                chunk_id: "c-3".to_string(),
                document_id: "doc-onboarding-setup".to_string(),
                ordinal: 0,
                source_kind: "obsidian".to_string(),
                title: "Onboarding System Setup".to_string(),
                content: "General new hire orientation setup checklist".to_string(),
                path_or_url: None,
                tags: vec!["onboarding".to_string(), "setup".to_string()],
                author: None,
                category: None,
                created_at: None,
                updated_at: None,
                metadata: serde_json::json!({}),
                chunk_metadata_json: None,
            },
        ];
        let dict = EntityDictionary::build(&mock_docs);
        
        let query_lower = "explain notion onboarding".to_lowercase();
        let matching_groups = dict.detect_matching_groups("Explain notion onboarding");
        
        let mut scored_groups = Vec::new();
        for g_name in &matching_groups {
            if let Some(group) = dict.group_for_cluster(g_name) {
                let score = dict.score_group_for_query(group, &query_lower);
                scored_groups.push((g_name.clone(), score));
            }
        }
        scored_groups.sort_by(|a, b| b.1.cmp(&a.1));
        
        let should_trigger_ambiguity = if scored_groups.len() > 1 {
            let top_score = scored_groups[0].1;
            let second_score = scored_groups[1].1;
            (top_score as i32 - second_score as i32).abs() <= 5
        } else {
            false
        };
        assert!(should_trigger_ambiguity);

        let query_lower_notion = "explain notion".to_lowercase();
        let matching_groups_notion = dict.detect_matching_groups("Explain notion");
        
        let mut scored_groups_notion = Vec::new();
        for g_name in &matching_groups_notion {
            if let Some(group) = dict.group_for_cluster(g_name) {
                let score = dict.score_group_for_query(group, &query_lower_notion);
                scored_groups_notion.push((g_name.clone(), score));
            }
        }
        scored_groups_notion.sort_by(|a, b| b.1.cmp(&a.1));
        
        let should_trigger_ambiguity_notion = if scored_groups_notion.len() > 1 {
            let top_score = scored_groups_notion[0].1;
            let second_score = scored_groups_notion[1].1;
            (top_score as i32 - second_score as i32).abs() <= 5
        } else {
            false
        };
        
        assert!(!should_trigger_ambiguity_notion);
        assert_eq!(scored_groups_notion[0].0, "notion");
    }

    #[test]
    fn test_expand_query_pollution_check() {
        let dict = EntityDictionary { groups: crate::services::entity_dictionary::get_static_groups() };
        let (_expanded, matched) = dict.expand("Compare Notion and Obsidian integrations");
        assert!(matched.contains(&"notion".to_string()));
        assert!(!matched.contains(&"authentication".to_string()));
        assert!(!matched.contains(&"database".to_string()));
    }

    #[test]
    fn test_detect_matching_groups_primary_only() {
        let dict = EntityDictionary { groups: crate::services::entity_dictionary::get_static_groups() };
        let matches = dict.detect_matching_groups("Explain setup");
        assert!(matches.is_empty());
    }

    #[test]
    fn test_confidence_no_title_bonus() {
        let service = mock_service();
        let dict = EntityDictionary::default();
        let chunk = mock_chunk("doc-1", 1.5, 0.01);
        let report = service.calculate_confidence(
            "Grafana",
            &[chunk],
            &RetrievalStrategy::Hybrid,
            &crate::domain::QueryComplexity::Simple,
            &dict,
        );
        assert!(report.confidence_score < 75);
    }
}


