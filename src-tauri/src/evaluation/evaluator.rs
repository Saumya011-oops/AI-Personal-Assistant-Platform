/// evaluation/evaluator.rs
///
/// Multi-Dimensional Evaluator — scores an ExecutionTrace across 7 independent
/// dimensions and produces a full EvalResult with per-claim grounding, root
/// cause attribution, and fix proposals.
///
/// Hallucination detection uses a three-stage hybrid verifier:
///   Stage 1: Exact grounding check (deterministic, fast)
///   Stage 2: Semantic similarity via Ollama embeddings (cosine ≥ threshold)
///   Stage 3: LLM-as-judge (Groq) ONLY for borderline cases where stages 1 & 2
///            cannot reach a confident verdict.
///
/// The LLM is never the primary source of truth — it is a last resort.

use std::sync::Arc;

use anyhow::Result;
use tracing::{info, warn};

use crate::services::groq::GroqService;
use crate::services::ollama::OllamaService;

use super::types::*;

// ──────────────────────────────────────────────────────────────────────────────
// Thresholds
// ──────────────────────────────────────────────────────────────────────────────

/// Cosine similarity threshold above which a claim is considered semantically
/// supported (Stage 2). Below this, the claim is sent to LLM judge (Stage 3).
/// Lowered from 0.72 → 0.68 to avoid penalizing valid paraphrases.
const SEMANTIC_SUPPORT_THRESHOLD: f32 = 0.68;

/// Cosine similarity threshold below which a claim is definitively unsupported
/// without calling the LLM (avoids LLM round-trip for clearly absent claims).
const SEMANTIC_REJECT_THRESHOLD: f32 = 0.30;

/// Production-readiness thresholds per dimension (0–100).
const THRESHOLD_RETRIEVAL: f32 = 95.0;
const THRESHOLD_MEMORY: f32 = 95.0;
const THRESHOLD_PROMPT: f32 = 95.0;
const THRESHOLD_ANSWER: f32 = 95.0;
const THRESHOLD_HALLUCINATION: f32 = 100.0; // 100 = zero hallucinations
const THRESHOLD_CITATION: f32 = 100.0;
const THRESHOLD_GROUNDING: f32 = 100.0;

/// Expected prompt section order markers.
const PROMPT_SECTION_ORDER: &[&str] = &[
    "Conversation Summary",
    "Long-Term Memories",
    "Recent Episodes",
    "Recent Conversation Messages",
    "Retrieved RAG Documents",
    "Current User Message",
];

// ──────────────────────────────────────────────────────────────────────────────
// Evaluator
// ──────────────────────────────────────────────────────────────────────────────

pub struct Evaluator {
    ollama: OllamaService,
    groq: GroqService,
}

impl Evaluator {
    pub fn new(ollama: OllamaService, groq: GroqService) -> Self {
        Self { ollama, groq }
    }

    /// Construct an Evaluator with no-op service URLs for unit testing.
    /// The pure scoring methods do not call Ollama or Groq.
    #[cfg(test)]
    pub(crate) fn new_for_test() -> Self {
        Self::new(
            OllamaService::new(
                "http://localhost:11434".to_string(),
                "nomic-embed-text".to_string(),
            ),
            GroqService::new(
                None,
                None,
                None,
                "http://localhost".to_string(),
                "llama3-8b-8192".to_string(),
                "llama3-8b-8192".to_string(),
            ),
        )
    }

    /// Main entry point — evaluates a trace against its test case's ground truth.
    pub async fn evaluate(&self, test: &TestCase, trace: &ExecutionTrace) -> EvalResult {
        // If the pipeline itself errored, short-circuit with an error result.
        if let Some(ref err) = trace.error {
            return self.error_result(test, trace, err);
        }

        // ── Dimension 1: Retrieval ────────────────────────────────────────────
        let retrieval_score = self.score_retrieval(test, trace);

        // ── Dimension 2: Memory ───────────────────────────────────────────────
        let memory_score = self.score_memory(test, trace);

        // ── Dimension 3: Prompt Assembly ──────────────────────────────────────
        let prompt_score = self.score_prompt_assembly(trace);

        // ── Dimension 4: Answer Quality ───────────────────────────────────────
        let answer_score = self.score_answer_quality(test, trace);

        // ── Dimension 5 & 7: Per-claim grounding (drives both hallucination
        //    and grounding scores) ────────────────────────────────────────────
        let claim_verifications = self.verify_claims(test, trace).await;

        let hallucination_score = self.score_hallucination(&claim_verifications);
        let grounding_score = self.score_grounding(&claim_verifications);

        // ── Dimension 6: Citation Accuracy ────────────────────────────────────
        let citation_score = self.score_citations(trace);

        let scorecard = EvalScorecard {
            retrieval: retrieval_score,
            memory: memory_score,
            prompt_assembly: prompt_score,
            answer_quality: answer_score,
            hallucination: hallucination_score,
            citation_accuracy: citation_score,
            grounding: grounding_score,
        };

        let passed = scorecard.production_ready();

        // ── Root cause analysis ───────────────────────────────────────────────
        let root_causes = if !passed {
            self.identify_root_causes(test, trace, &scorecard, &claim_verifications)
        } else {
            vec![]
        };

        // ── Fix proposals ─────────────────────────────────────────────────────
        let fix_proposals = self.propose_fixes(&root_causes);

        EvalResult {
            test_id: test.id.clone(),
            suite: test.suite.clone(),
            category: test.category.clone(),
            query: test.query.clone(),
            passed,
            scorecard,
            claim_verifications,
            root_causes,
            fix_proposals,
            trace: trace.clone(),
            is_regression: false, // set by regression runner
            is_improvement: false,
        }
    }

    // ── Dimension 1: Retrieval ────────────────────────────────────────────────

    pub(crate) fn score_retrieval(&self, test: &TestCase, trace: &ExecutionTrace) -> DimensionScore {
        let gt = &test.ground_truth;
        let mut score: f32 = 100.0;
        let mut details = Vec::new();

        // 1a. Status check
        if let Some(ref conf) = trace.confidence {
            if !gt.expected_statuses.is_empty()
                && !gt.expected_statuses.contains(&conf.status)
            {
                score -= 30.0;
                details.push(format!(
                    "Status mismatch: expected one of {:?}, got '{}'",
                    gt.expected_statuses, conf.status
                ));
            } else {
                details.push(format!("Status OK: '{}'", conf.status));
            }
        } else {
            score -= 10.0;
            details.push("No confidence report returned".to_string());
        }

        // 1b. Required document recall
        let retrieved_titles_lower: Vec<String> = trace
            .post_rerank_chunks
            .iter()
            .map(|c| c.document_title.to_lowercase())
            .collect();

        for req_doc in &gt.required_doc_ids {
            let req_lower = req_doc.to_lowercase();
            let req_spaced = req_lower.replace('_', " ");
            let found = retrieved_titles_lower
                .iter()
                .any(|t| t.contains(&req_lower) || t.contains(&req_spaced));
            if !found {
                score -= 20.0;
                details.push(format!(
                    "Required document not retrieved: '{}'",
                    req_doc
                ));
            } else {
                details.push(format!("Required document found: '{}'", req_doc));
            }
        }

        // 1c. Entity recall in retrieved chunks
        let all_chunk_text: String = trace
            .post_rerank_chunks
            .iter()
            .map(|c| c.document_title.to_lowercase() + " " + &c.content.to_lowercase())
            .collect::<Vec<_>>()
            .join(" ");

        for entity in &gt.required_entities {
            let entity_lower = entity.to_lowercase();
            if !all_chunk_text.contains(&entity_lower) {
                score -= 15.0;
                details.push(format!(
                    "Required entity '{}' not found in retrieved chunks",
                    entity
                ));
            } else {
                details.push(format!("Entity '{}' present in chunks", entity));
            }
        }

        // 1d. Citation count
        if trace.citations.len() < gt.min_citations {
            score -= 20.0;
            details.push(format!(
                "Insufficient citations: need ≥ {}, got {}",
                gt.min_citations,
                trace.citations.len()
            ));
        }

        // 1e. No duplicate chunks
        let chunk_ids: Vec<&str> = trace
            .post_rerank_chunks
            .iter()
            .map(|c| c.chunk_id.as_str())
            .collect();
        let unique_count = chunk_ids.iter().collect::<std::collections::HashSet<_>>().len();
        if unique_count < chunk_ids.len() {
            score -= 10.0;
            details.push(format!(
                "Duplicate chunks detected: {} total, {} unique",
                chunk_ids.len(),
                unique_count
            ));
        }

        DimensionScore::new(score.max(0.0), THRESHOLD_RETRIEVAL, details)
    }

    // ── Dimension 2: Memory ───────────────────────────────────────────────────

    pub(crate) fn score_memory(&self, test: &TestCase, trace: &ExecutionTrace) -> DimensionScore {
        let gt = &test.ground_truth;
        let mut score: f32 = 100.0;
        let mut details = Vec::new();

        if gt.required_memory_content.is_empty() && test.memory_fixtures.is_empty() {
            // No memory assertions — skip with full score
            details.push("No memory assertions in this test case".to_string());
            return DimensionScore::new(100.0, THRESHOLD_MEMORY, details);
        }

        // 2a. Required memory content recall
        let recalled_text: String = trace
            .recalled_memories
            .iter()
            .map(|rm| rm.memory.content.to_lowercase())
            .collect::<Vec<_>>()
            .join(" ");

        for req_content in &gt.required_memory_content {
            let req_lower = req_content.to_lowercase();
            if !recalled_text.contains(&req_lower) {
                score -= 30.0;
                details.push(format!(
                    "Required memory content not recalled: '{}'",
                    req_content
                ));
            } else {
                details.push(format!(
                    "Required memory content recalled: '{}'",
                    req_content
                ));
            }
        }

        // 2b. No duplicate memories recalled
        let memory_ids: Vec<&str> = trace
            .recalled_memories
            .iter()
            .map(|rm| rm.memory.id.as_str())
            .collect();
        let unique_mems = memory_ids
            .iter()
            .collect::<std::collections::HashSet<_>>()
            .len();
        if unique_mems < memory_ids.len() {
            score -= 15.0;
            details.push(format!(
                "Duplicate memories recalled: {} total, {} unique",
                memory_ids.len(),
                unique_mems
            ));
        }

        // 2c. Stale memory not surfaced (for stale test cases)
        if test.category == TestCategory::StaleMemoryRejection {
            // Verify: the highest-ranked recalled memory is NOT the stale fixture
            let stale_ids: Vec<&str> = test
                .memory_fixtures
                .iter()
                .filter(|f| f.is_stale)
                .map(|f| f.id.as_str())
                .collect();

            if let Some(top_memory) = trace.recalled_memories.first() {
                if stale_ids.contains(&top_memory.memory.id.as_str()) {
                    score -= 40.0;
                    details.push(format!(
                        "Stale memory ranked first (id: {}). Freshness weighting failure.",
                        top_memory.memory.id
                    ));
                } else {
                    details.push("Fresh memory correctly ranked above stale".to_string());
                }
            }
        }

        // 2d. Irrelevant memory not surfaced (for irrelevant-rejection test)
        if test.category == TestCategory::IrrelevantMemoryRejection {
            let irrelevant_ids: Vec<&str> = test
                .memory_fixtures
                .iter()
                .map(|f| f.id.as_str())
                .collect();

            for rm in &trace.recalled_memories {
                if irrelevant_ids.contains(&rm.memory.id.as_str()) {
                    score -= 30.0;
                    details.push(format!(
                        "Irrelevant memory surfaced (id: {}, content: '{}')",
                        rm.memory.id,
                        &rm.memory.content[..rm.memory.content.len().min(60)]
                    ));
                }
            }
            if score >= 100.0 {
                details.push("Irrelevant memories correctly filtered out".to_string());
            }
        }

        // 2e. Memory ranking sanity — verify descending final_score order
        let scores: Vec<f64> = trace
            .recalled_memories
            .iter()
            .map(|rm| rm.final_score)
            .collect();
        let is_sorted = scores.windows(2).all(|w| w[0] >= w[1]);
        if !is_sorted {
            score -= 10.0;
            details.push("Memory ranking is not in descending score order".to_string());
        }

        DimensionScore::new(score.max(0.0), THRESHOLD_MEMORY, details)
    }

    // ── Dimension 3: Prompt Assembly ──────────────────────────────────────────

    pub(crate) fn score_prompt_assembly(&self, trace: &ExecutionTrace) -> DimensionScore {
        let mut score: f32 = 100.0;
        let mut details = Vec::new();

        // If prompt was not captured (retrieval service assembles internally),
        // use the final answer as a proxy for ordering checks.
        // Full prompt capture requires a refactor of retrieval.rs (tracked as
        // improvement, not deducted here unless answer is absent).
        let prompt = &trace.prompt_assembled;

        if prompt.is_empty() {
            // Can't validate ordering without the prompt — award partial
            details.push(
                "Prompt not captured in trace (internal to RetrievalService). \
                 Ordering validation skipped. Improvement: expose assembled prompt."
                    .to_string(),
            );
            score -= 5.0; // small deduction for observability gap
        } else {
            // Collect (section_name, byte_position) for every section found.
            let present_sections: Vec<(&str, usize)> = PROMPT_SECTION_ORDER
                .iter()
                .filter_map(|s| prompt.find(s).map(|pos| (*s, pos)))
                .collect();

            // Check all pairs (i, j) where i < j in canonical order.
            // If section[i] appears at a later byte position than section[j],
            // that is an ordering violation — catches non-adjacent inversions.
            let mut order_ok = true;
            'outer: for i in 0..present_sections.len() {
                for j in (i + 1)..present_sections.len() {
                    let (name_i, pos_i) = present_sections[i];
                    let (name_j, pos_j) = present_sections[j];
                    if pos_i > pos_j {
                        order_ok = false;
                        details.push(format!(
                            "Prompt ordering violation: '{}' (pos {}) appears after '{}' (pos {})",
                            name_i, pos_i, name_j, pos_j
                        ));
                        score -= 20.0;
                        break 'outer;
                    }
                }
            }
            if order_ok {
                details.push("Prompt section ordering is correct".to_string());
            }

            // 3b. Duplicate context blocks — count the canonical '### SectionName'
            // headers only, not arbitrary occurrences of the name inside content
            // (e.g., a LLM-generated summary may itself contain 'Conversation Summary:')
            let sections_found: Vec<&str> = PROMPT_SECTION_ORDER
                .iter()
                .filter(|s| {
                    let header = format!("### {}", s);
                    prompt.matches(header.as_str()).count() > 1
                })
                .copied()
                .collect();
            if !sections_found.is_empty() {
                score -= 25.0;
                details.push(format!(
                    "Duplicate prompt sections detected: {:?}",
                    sections_found
                ));
            }

            // 3c. Overflow check — rough token estimate (1 token ≈ 4 chars)
            let estimated_tokens = prompt.len() / 4;
            if estimated_tokens > 30_000 {
                score -= 20.0;
                details.push(format!(
                    "Prompt overflow risk: estimated {} tokens (> 30k limit)",
                    estimated_tokens
                ));
            } else {
                details.push(format!(
                    "Prompt size OK: ~{} estimated tokens",
                    estimated_tokens
                ));
            }
        }

        DimensionScore::new(score.max(0.0), THRESHOLD_PROMPT, details)
    }

    // ── Dimension 4: Answer Quality ───────────────────────────────────────────

    pub(crate) fn score_answer_quality(&self, test: &TestCase, trace: &ExecutionTrace) -> DimensionScore {
        let gt = &test.ground_truth;
        let answer_lower = trace.final_answer.to_lowercase();
        let mut score: f32 = 100.0;
        let mut details = Vec::new();

        if trace.final_answer.is_empty() {
            return DimensionScore::new(0.0, THRESHOLD_ANSWER, vec!["Empty answer".to_string()]);
        }

        // 4a. Required answer keywords
        for kw in &gt.required_answer_keywords {
            if !answer_lower.contains(&kw.to_lowercase()) {
                score -= 15.0;
                details.push(format!("Required keyword '{}' missing from answer", kw));
            } else {
                details.push(format!("Required keyword '{}' present", kw));
            }
        }

        // 4b. Forbidden terms
        for term in &gt.forbidden_terms {
            if answer_lower.contains(&term.to_lowercase()) {
                score -= 30.0;
                details.push(format!("Forbidden term '{}' found in answer", term));
            }
        }

        // 4c. Answer characteristics
        for characteristic in &gt.answer_characteristics {
            match characteristic {
                AnswerCharacteristic::ContainsComparison => {
                    let comparison_words = ["however", "whereas", "on the other hand",
                        "in contrast", "compared to", "differs", "while", "unlike"];
                    let has_comparison = comparison_words.iter().any(|w| answer_lower.contains(w));
                    if !has_comparison {
                        score -= 10.0;
                        details.push("Answer should contain a comparison but does not".to_string());
                    } else {
                        details.push("Comparison language present in answer".to_string());
                    }
                }
                AnswerCharacteristic::AcknowledgesUncertainty => {
                    let uncertainty_words = ["don't have", "not found", "no information",
                        "unable to find", "i'm not aware", "i cannot", "not available",
                        "doesn't appear", "no data", "sorry"];
                    let acknowledges = uncertainty_words.iter().any(|w| answer_lower.contains(w));
                    if !acknowledges {
                        score -= 15.0;
                        details.push(
                            "Answer should acknowledge uncertainty but makes confident claims"
                                .to_string(),
                        );
                    } else {
                        details.push("Answer correctly acknowledges uncertainty".to_string());
                    }
                }
                AnswerCharacteristic::NoClaims => {
                    // If answer makes factual claims (contains "is", "are", "was"), deduct
                    let claim_words = [" is ", " are ", " was ", " were "];
                    let has_claims = claim_words.iter().any(|w| answer_lower.contains(w));
                    if has_claims {
                        score -= 10.0;
                        details.push(
                            "Answer makes factual claims when it should not".to_string(),
                        );
                    }
                }
                AnswerCharacteristic::StepByStep => {
                    let step_indicators = ["1.", "2.", "step 1", "first,", "then,", "finally,"];
                    let has_steps = step_indicators.iter().any(|w| answer_lower.contains(w));
                    if !has_steps {
                        score -= 10.0;
                        details.push(
                            "Answer should include step-by-step instructions".to_string(),
                        );
                    }
                }
                AnswerCharacteristic::ContainsDate => {
                    let date_patterns = [
                        "january", "february", "march", "april", "may", "june",
                        "july", "august", "september", "october", "november", "december",
                        "2024", "2025", "2026", "today", "yesterday", "recently",
                    ];
                    let has_date = date_patterns.iter().any(|p| answer_lower.contains(p));
                    if !has_date {
                        details.push("No date reference found (expected ContainsDate)".to_string());
                        // Not a hard failure — date may not be in data
                    }
                }
            }
        }

        // 4d. Minimum length check
        if trace.final_answer.split_whitespace().count() < 10 {
            score -= 20.0;
            details.push(format!(
                "Answer too short: {} words",
                trace.final_answer.split_whitespace().count()
            ));
        }

        DimensionScore::new(score.max(0.0), THRESHOLD_ANSWER, details)
    }

    // ── Dimension 5/7: Per-claim verification (three-stage hybrid) ────────────

    pub async fn verify_claims(
        &self,
        test: &TestCase,
        trace: &ExecutionTrace,
    ) -> Vec<ClaimVerification> {
        if trace.final_answer.is_empty() {
            return vec![];
        }

        // Tokenize answer into sentences (simple split on '. ', '! ', '? ')
        let sentences: Vec<String> = tokenize_sentences(&trace.final_answer);

        // Build evidence corpus from retrieved chunks + recalled memories
        let chunk_evidence: Vec<(String, String)> = trace
            .post_rerank_chunks
            .iter()
            .map(|c| (c.chunk_id.clone(), c.content.clone() + " " + &c.document_title))
            .collect();

        let memory_evidence: Vec<(String, String)> = trace
            .recalled_memories
            .iter()
            .map(|rm| (rm.memory.id.clone(), rm.memory.content.clone()))
            .collect();

        let mut results = Vec::new();

        // Get embeddings for all sentences + evidence (batch call)
        let all_texts: Vec<String> = sentences
            .iter()
            .chain(chunk_evidence.iter().map(|(_, c)| c))
            .chain(memory_evidence.iter().map(|(_, c)| c))
            .cloned()
            .collect();

        let embeddings_result = self.ollama.generate_embeddings(&all_texts).await;

        let embeddings = match embeddings_result {
            Ok(embs) => embs,
            Err(e) => {
                warn!("Embedding generation failed for claim verification: {:?}", e);
                // Fall back to exact-match only
                for sentence in &sentences {
                    results.push(
                        self.exact_grounding_check(sentence, &chunk_evidence, &memory_evidence)
                            .await,
                    );
                }
                return results;
            }
        };

        let n_sentences = sentences.len();
        let sentence_embeddings = &embeddings[..n_sentences];
        let evidence_embeddings = &embeddings[n_sentences..];

        let n_chunks = chunk_evidence.len();
        let chunk_embeddings = &evidence_embeddings[..n_chunks];
        let mem_embeddings = &evidence_embeddings[n_chunks..];

        for (i, sentence) in sentences.iter().enumerate() {
            // Skip trivially short or non-factual sentences
            if is_non_factual_sentence(sentence) {
                continue;
            }

            let claim_embedding = &sentence_embeddings[i];

            // Stage 1: Exact grounding
            let exact = self
                .exact_grounding_check(sentence, &chunk_evidence, &memory_evidence)
                .await;
            if exact.support == ClaimSupport::Supported {
                results.push(exact);
                continue;
            }

            // Stage 2: Semantic similarity
            let semantic = semantic_grounding_check(
                sentence,
                claim_embedding,
                &chunk_evidence,
                chunk_embeddings,
                &memory_evidence,
                mem_embeddings,
            );

            match &semantic.support {
                ClaimSupport::Supported | ClaimSupport::Hallucinated => {
                    // Confident result from Stage 2
                    results.push(semantic);
                }
                ClaimSupport::PartiallySupported | ClaimSupport::Unsupported => {
                    // Stage 3: LLM-as-judge for borderline cases
                    let judge_result = self
                        .llm_judge_claim(sentence, &chunk_evidence, &memory_evidence, &semantic)
                        .await;
                    results.push(judge_result);
                }
            }
        }

        results
    }

    // Stage 1: Exact grounding (deterministic string matching)
    async fn exact_grounding_check(
        &self,
        claim: &str,
        chunk_evidence: &[(String, String)],
        memory_evidence: &[(String, String)],
    ) -> ClaimVerification {
        let claim_lower = claim.to_lowercase();

        // Extract key noun phrases from the claim (simple heuristic: words > 4 chars)
        let key_terms: Vec<&str> = claim_lower
            .split_whitespace()
            .filter(|w| w.len() > 4 && !STOP_WORDS.contains(w))
            .collect();

        if key_terms.is_empty() {
            return ClaimVerification {
                claim: claim.to_string(),
                support: ClaimSupport::Unsupported,
                supporting_evidence: None,
                evidence_source: None,
                determined_by: "exact_match".to_string(),
                similarity_score: None,
            };
        }

        // Check chunks
        for (chunk_id, chunk_text) in chunk_evidence {
            let chunk_lower = chunk_text.to_lowercase();
            let matches = key_terms.iter().filter(|t| chunk_lower.contains(**t)).count();
            let coverage = matches as f32 / key_terms.len() as f32;

            if coverage >= 0.7 {
                return ClaimVerification {
                    claim: claim.to_string(),
                    support: ClaimSupport::Supported,
                    supporting_evidence: Some(chunk_text[..chunk_text.len().min(200)].to_string()),
                    evidence_source: Some(chunk_id.clone()),
                    determined_by: "exact_match".to_string(),
                    similarity_score: Some(coverage),
                };
            }
        }

        // Check memories
        for (mem_id, mem_text) in memory_evidence {
            let mem_lower = mem_text.to_lowercase();
            let matches = key_terms.iter().filter(|t| mem_lower.contains(**t)).count();
            let coverage = matches as f32 / key_terms.len() as f32;

            if coverage >= 0.7 {
                return ClaimVerification {
                    claim: claim.to_string(),
                    support: ClaimSupport::Supported,
                    supporting_evidence: Some(mem_text.clone()),
                    evidence_source: Some(format!("memory:{}", mem_id)),
                    determined_by: "exact_match".to_string(),
                    similarity_score: Some(coverage),
                };
            }
        }

        ClaimVerification {
            claim: claim.to_string(),
            support: ClaimSupport::Unsupported,
            supporting_evidence: None,
            evidence_source: None,
            determined_by: "exact_match".to_string(),
            similarity_score: None,
        }
    }

    // Stage 3: LLM-as-judge (only for borderline Stage 2 cases)
    async fn llm_judge_claim(
        &self,
        claim: &str,
        chunk_evidence: &[(String, String)],
        memory_evidence: &[(String, String)],
        stage2_result: &ClaimVerification,
    ) -> ClaimVerification {
        // Build a concise evidence context (limit tokens)
        let evidence_snippets: Vec<String> = chunk_evidence
            .iter()
            .take(5)
            .map(|(id, text)| {
                format!("[CHUNK {}] {}", &id[..id.len().min(12)], &text[..text.len().min(300)])
            })
            .chain(memory_evidence.iter().take(3).map(|(id, text)| {
                format!("[MEMORY {}] {}", &id[..id.len().min(12)], &text[..text.len().min(200)])
            }))
            .collect();

        let evidence_context = evidence_snippets.join("\n\n");

        let system_prompt = "You are a factual grounding verifier. Your task is to determine whether a \
            claim made in an AI answer is supported by the provided evidence. \
            Respond with a JSON object containing exactly: \
            {\"verdict\": \"SUPPORTED\" | \"PARTIALLY_SUPPORTED\" | \"UNSUPPORTED\" | \"HALLUCINATED\", \
             \"explanation\": \"one sentence reason\", \
             \"evidence_snippet\": \"relevant quote from evidence or null\"}. \
            Rules: \
            SUPPORTED = claim is directly supported by evidence. \
            PARTIALLY_SUPPORTED = claim is hinted at but not fully confirmed. \
            UNSUPPORTED = claim is absent from evidence but may be a valid inference. \
            HALLUCINATED = claim directly contradicts or fabricates information. \
            The LLM should NEVER hallucinate — when uncertain, prefer UNSUPPORTED over HALLUCINATED.";

        let user_prompt = format!(
            "CLAIM TO VERIFY:\n\"{}\"\n\nEVIDENCE:\n{}\n\nVerdict:",
            claim, evidence_context
        );

        match self.ollama.chat_json(system_prompt, &user_prompt).await {
            Ok(json) => {
                let verdict_str = json["verdict"].as_str().unwrap_or("UNSUPPORTED");
                let explanation = json["explanation"].as_str().unwrap_or("").to_string();
                let evidence_snippet = json["evidence_snippet"].as_str().map(|s| s.to_string());

                let support = match verdict_str {
                    "SUPPORTED" => ClaimSupport::Supported,
                    "PARTIALLY_SUPPORTED" => ClaimSupport::PartiallySupported,
                    "HALLUCINATED" => ClaimSupport::Hallucinated,
                    _ => ClaimSupport::Unsupported,
                };

                ClaimVerification {
                    claim: claim.to_string(),
                    support,
                    supporting_evidence: evidence_snippet
                        .or(stage2_result.supporting_evidence.clone()),
                    evidence_source: stage2_result.evidence_source.clone(),
                    determined_by: "llm_judge".to_string(),
                    similarity_score: stage2_result.similarity_score,
                }
            }
            Err(e) => {
                warn!("LLM judge call failed for claim '{}': {:?}", &claim[..claim.len().min(60)], e);
                // Fall back to Stage 2 result
                ClaimVerification {
                    claim: claim.to_string(),
                    support: stage2_result.support.clone(),
                    determined_by: "llm_judge_fallback_stage2".to_string(),
                    ..stage2_result.clone()
                }
            }
        }
    }

    // ── Dimension 5: Hallucination Score ─────────────────────────────────────

    pub(crate) fn score_hallucination(&self, claims: &[ClaimVerification]) -> DimensionScore {
        if claims.is_empty() {
            return DimensionScore::new(100.0, THRESHOLD_HALLUCINATION, vec!["No claims to verify".to_string()]);
        }

        let hallucinated: Vec<&ClaimVerification> = claims
            .iter()
            .filter(|c| c.support == ClaimSupport::Hallucinated)
            .collect();

        let mut details = vec![];
        if hallucinated.is_empty() {
            details.push(format!("All {} claims are grounded", claims.len()));
            DimensionScore::new(100.0, THRESHOLD_HALLUCINATION, details)
        } else {
            for hc in &hallucinated {
                details.push(format!(
                    "HALLUCINATED: \"{}\" (determined by: {})",
                    &hc.claim[..hc.claim.len().min(120)],
                    hc.determined_by
                ));
            }
            // Score: 0 points for any hallucination (binary — must be zero)
            DimensionScore::new(0.0, THRESHOLD_HALLUCINATION, details)
        }
    }

    // ── Dimension 6: Citation Accuracy ────────────────────────────────────────

    pub(crate) fn score_citations(&self, trace: &ExecutionTrace) -> DimensionScore {
        if trace.citations.is_empty() {
            // If no citations required and none given, that's fine
            return DimensionScore::new(
                100.0,
                THRESHOLD_CITATION,
                vec!["No citations to validate".to_string()],
            );
        }

        let retrieved_chunk_ids: std::collections::HashSet<&str> = trace
            .post_rerank_chunks
            .iter()
            .map(|c| c.chunk_id.as_str())
            .collect();

        let mut score: f32 = 100.0;
        let mut details = Vec::new();

        for citation in &trace.citations {
            if citation.chunk_id.is_empty() {
                // Allow empty chunk_id (some citations may be memory-based)
                continue;
            }
            if !retrieved_chunk_ids.contains(citation.chunk_id.as_str()) {
                score = 0.0; // Any fabricated citation is a full failure
                details.push(format!(
                    "FABRICATED CITATION: chunk_id '{}' not in retrieved set (source: '{}')",
                    citation.chunk_id, citation.source_document
                ));
            } else {
                details.push(format!(
                    "Citation verified: chunk_id '{}' in retrieved set",
                    &citation.chunk_id[..citation.chunk_id.len().min(16)]
                ));
            }
        }

        DimensionScore::new(score, THRESHOLD_CITATION, details)
    }

    // ── Dimension 7: Grounding Score ──────────────────────────────────────────

    pub(crate) fn score_grounding(&self, claims: &[ClaimVerification]) -> DimensionScore {
        if claims.is_empty() {
            return DimensionScore::new(
                100.0,
                THRESHOLD_GROUNDING,
                vec!["No claims to verify".to_string()],
            );
        }

        let supported = claims
            .iter()
            .filter(|c| {
                c.support == ClaimSupport::Supported
                    || c.support == ClaimSupport::PartiallySupported
            })
            .count();

        let grounding_pct = (supported as f32 / claims.len() as f32) * 100.0;

        let mut details = vec![format!(
            "{}/{} claims grounded ({:.1}%)",
            supported,
            claims.len(),
            grounding_pct
        )];

        for c in claims {
            let tag = match &c.support {
                ClaimSupport::Supported => "✅",
                ClaimSupport::PartiallySupported => "⚠️",
                ClaimSupport::Unsupported => "❌",
                ClaimSupport::Hallucinated => "🚨",
            };
            details.push(format!(
                "  {} [{}] \"{}\"",
                tag,
                c.support,
                &c.claim[..c.claim.len().min(100)]
            ));
        }

        DimensionScore::new(grounding_pct, THRESHOLD_GROUNDING, details)
    }

    // ── Root Cause Analysis ───────────────────────────────────────────────────

    fn identify_root_causes(
        &self,
        test: &TestCase,
        trace: &ExecutionTrace,
        scorecard: &EvalScorecard,
        claims: &[ClaimVerification],
    ) -> Vec<FailureRootCause> {
        let mut causes = Vec::new();

        // Retrieval failure
        if !scorecard.retrieval.passed {
            let retrieved_titles: Vec<String> = trace
                .post_rerank_chunks
                .iter()
                .map(|c| c.document_title.clone())
                .take(5)
                .collect();

            if trace.post_rerank_chunks.is_empty() {
                causes.push(FailureRootCause::EmptyRetrieval);
            } else {
                let missing_docs: Vec<String> = test
                    .ground_truth
                    .required_doc_ids
                    .iter()
                    .filter(|req| {
                        let req_lower = req.to_lowercase();
                        !retrieved_titles.iter().any(|t| {
                            t.to_lowercase().contains(&req_lower)
                        })
                    })
                    .cloned()
                    .collect();

                if !missing_docs.is_empty() {
                    causes.push(FailureRootCause::RankingFailure {
                        expected: missing_docs,
                        got: retrieved_titles,
                    });
                }

                let missing_entities: Vec<String> = test
                    .ground_truth
                    .required_entities
                    .iter()
                    .filter(|e| {
                        let e_lower = e.to_lowercase();
                        !trace.post_rerank_chunks.iter().any(|c| {
                            c.content.to_lowercase().contains(&e_lower)
                                || c.document_title.to_lowercase().contains(&e_lower)
                        })
                    })
                    .cloned()
                    .collect();

                if !missing_entities.is_empty() {
                    causes.push(FailureRootCause::EntityRecallGap {
                        missing: missing_entities,
                    });
                }
            }
        }

        // Memory failure
        if !scorecard.memory.passed {
            let missing_content: Vec<String> = test
                .ground_truth
                .required_memory_content
                .iter()
                .filter(|req| {
                    let req_lower = req.to_lowercase();
                    !trace.recalled_memories.iter().any(|rm| {
                        rm.memory.content.to_lowercase().contains(&req_lower)
                    })
                })
                .cloned()
                .collect();

            if !missing_content.is_empty() {
                causes.push(FailureRootCause::MemoryRecallError {
                    detail: format!("Missing memory content: {:?}", missing_content),
                });
            }
        }

        // Prompt assembly failure
        if !scorecard.prompt_assembly.passed {
            for detail in &scorecard.prompt_assembly.details {
                if detail.contains("ordering violation")
                    || detail.contains("Duplicate")
                    || detail.contains("overflow")
                {
                    causes.push(FailureRootCause::PromptAssemblyError {
                        detail: detail.clone(),
                    });
                }
            }
        }

        // Hallucination
        for claim in claims {
            if claim.support == ClaimSupport::Hallucinated {
                causes.push(FailureRootCause::HallucinatedClaim {
                    claim: claim.claim.clone(),
                });
            }
        }

        // Fabricated citations
        for detail in &scorecard.citation_accuracy.details {
            if detail.starts_with("FABRICATED CITATION:") {
                // Extract chunk_id from the detail string
                if let Some(id_start) = detail.find('\'') {
                    if let Some(id_end) = detail[id_start + 1..].find('\'') {
                        let chunk_id = detail[id_start + 1..id_start + 1 + id_end].to_string();
                        causes.push(FailureRootCause::FabricatedCitation { chunk_id });
                    }
                }
            }
        }

        // Answer quality failure
        if !scorecard.answer_quality.passed {
            let missing: Vec<String> = test
                .ground_truth
                .required_answer_keywords
                .iter()
                .filter(|kw| {
                    !trace.final_answer.to_lowercase().contains(&kw.to_lowercase())
                })
                .cloned()
                .collect();

            if !missing.is_empty() {
                causes.push(FailureRootCause::AnswerQualityFailure { missing });
            }
        }

        if causes.is_empty() && !scorecard.production_ready() {
            causes.push(FailureRootCause::Unknown {
                detail: "Score below threshold but specific cause not identified".to_string(),
            });
        }

        causes
    }

    // ── Fix Proposals ────────────────────────────────────────────────────────

    fn propose_fixes(&self, causes: &[FailureRootCause]) -> Vec<FixProposal> {
        let mut proposals = Vec::new();

        for cause in causes {
            match cause {
                FailureRootCause::EmptyRetrieval => {
                    proposals.push(FixProposal {
                        file: "src/services/retrieval.rs".to_string(),
                        function: Some("ask_assistant_with_mode".to_string()),
                        line_hint: None,
                        description:
                            "Empty retrieval: verify sparse fallback is activated when Qdrant \
                             returns 0 results. Check SparseRetrievalService::search() returns \
                             results and that the confidence gate does not suppress valid sparse hits."
                                .to_string(),
                        auto_applicable: false,
                        proposed_change:
                            "// Ensure sparse fallback triggers on empty dense results:\n\
                             if dense_results.is_empty() {\n\
                             \tlet sparse = sparse_service.search(&query, limit).await?;\n\
                             \t// merge sparse results before confidence check\n\
                             }"
                            .to_string(),
                    });
                }
                FailureRootCause::RankingFailure { expected, .. } => {
                    proposals.push(FixProposal {
                        file: "src/services/retrieval.rs".to_string(),
                        function: Some("hybrid_retrieve".to_string()),
                        line_hint: None,
                        description: format!(
                            "Documents {:?} should rank higher. Check RRF fusion weights and \
                             reranker threshold. Increasing dense weight or lowering reranker \
                             min_score may help.",
                            expected
                        ),
                        auto_applicable: false,
                        proposed_change:
                            "// Tune RRF weights: increase dense_weight from 0.6 to 0.7\n\
                             // Review reranker_min_score threshold"
                            .to_string(),
                    });
                }
                FailureRootCause::MemoryRecallError { detail } => {
                    proposals.push(FixProposal {
                        file: "src/services/memory/mod.rs".to_string(),
                        function: Some("retrieve_memories_for_query".to_string()),
                        line_hint: None,
                        description: format!(
                            "Memory recall failed: {}. \
                             Check Qdrant vector similarity threshold and ensure memory \
                             was correctly embedded during insertion.",
                            detail
                        ),
                        auto_applicable: false,
                        proposed_change:
                            "// Lower Qdrant similarity threshold for memory search\n\
                             // or increase candidate fetch limit (currently limit * 3)"
                            .to_string(),
                    });
                }
                FailureRootCause::PromptAssemblyError { detail } => {
                    proposals.push(FixProposal {
                        file: "src/services/prompt_builder.rs".to_string(),
                        function: Some("build_user_prompt".to_string()),
                        line_hint: None,
                        description: format!(
                            "Prompt assembly issue: {}. \
                             Review section ordering in PromptBuilder::build_user_prompt().",
                            detail
                        ),
                        auto_applicable: false,
                        proposed_change:
                            "// Verify the section push order matches spec:\n\
                             // convo_summary → long_term → episodes → recent → rag → query"
                            .to_string(),
                    });
                }
                FailureRootCause::HallucinatedClaim { claim } => {
                    proposals.push(FixProposal {
                        file: "src/services/retrieval.rs".to_string(),
                        function: Some("generate_answer".to_string()),
                        line_hint: None,
                        description: format!(
                            "Hallucinated claim: \"{}\". \
                             Strengthen the system prompt to instruct the LLM to only answer \
                             based on retrieved evidence. Add explicit 'DO NOT fabricate' \
                             instructions and a 'no relevant documents found' fallback.",
                            &claim[..claim.len().min(120)]
                        ),
                        auto_applicable: false,
                        proposed_change:
                            "// Add to system prompt:\n\
                             // \"IMPORTANT: Only use information from the provided documents.\n\
                             //  If the answer is not in the documents, say so explicitly.\n\
                             //  Do NOT make up information or use general knowledge.\""
                            .to_string(),
                    });
                }
                FailureRootCause::FabricatedCitation { chunk_id } => {
                    proposals.push(FixProposal {
                        file: "src/services/retrieval.rs".to_string(),
                        function: Some("parse_citations".to_string()),
                        line_hint: None,
                        description: format!(
                            "Fabricated citation (chunk_id: '{}'). \
                             Citation parser should validate that every cited chunk_id \
                             exists in the retrieved set before adding to response.",
                            chunk_id
                        ),
                        auto_applicable: true, // safe mechanical fix: add validation filter
                        proposed_change: format!(
                            "// Filter citations to only include retrieved chunk IDs:\n\
                             citations.retain(|c| retrieved_chunk_ids.contains(c.chunk_id.as_str()));"
                        ),
                    });
                }
                FailureRootCause::AnswerQualityFailure { missing } => {
                    proposals.push(FixProposal {
                        file: "src/services/retrieval.rs".to_string(),
                        function: Some("generate_answer".to_string()),
                        line_hint: None,
                        description: format!(
                            "Answer quality: keywords {:?} missing. \
                             Check if the relevant chunks were retrieved and whether the \
                             system prompt instructs the LLM to address all aspects of the query.",
                            missing
                        ),
                        auto_applicable: false,
                        proposed_change:
                            "// Review system prompt completeness instructions\n\
                             // and verify chunk content coverage for the topic"
                            .to_string(),
                    });
                }
                FailureRootCause::ExecutionError { message } => {
                    proposals.push(FixProposal {
                        file: "src/services/".to_string(),
                        function: None,
                        line_hint: None,
                        description: format!("Execution error: {}. Check service availability.", message),
                        auto_applicable: false,
                        proposed_change: "// Investigate service logs and connectivity".to_string(),
                    });
                }
                FailureRootCause::Unknown { detail } => {
                    proposals.push(FixProposal {
                        file: "evaluation/evaluator.rs".to_string(),
                        function: None,
                        line_hint: None,
                        description: format!("Unknown failure: {}. Manual investigation required.", detail),
                        auto_applicable: false,
                        proposed_change: "// Inspect the full ExecutionTrace for clues".to_string(),
                    });
                }
                FailureRootCause::EntityRecallGap { missing } => {
                    proposals.push(FixProposal {
                        file: "src/services/retrieval.rs".to_string(),
                        function: Some("hybrid_retrieve".to_string()),
                        line_hint: None,
                        description: format!(
                            "Entities {:?} not found in retrieved chunks. \
                             Check chunking strategy — these entities may be split across \
                             chunk boundaries. Consider increasing chunk overlap.",
                            missing
                        ),
                        auto_applicable: false,
                        proposed_change:
                            "// In chunker.rs: increase overlap from current value\n\
                             // e.g. chunk_overlap: 128 → 200 tokens"
                            .to_string(),
                    });
                }
            }
        }

        proposals
    }

    // ── Error result ─────────────────────────────────────────────────────────

    fn error_result(
        &self,
        test: &TestCase,
        trace: &ExecutionTrace,
        error: &str,
    ) -> EvalResult {
        let zero = DimensionScore::new(0.0, 95.0, vec![format!("Execution error: {}", error)]);
        EvalResult {
            test_id: test.id.clone(),
            suite: test.suite.clone(),
            category: test.category.clone(),
            query: test.query.clone(),
            passed: false,
            scorecard: EvalScorecard {
                retrieval: zero.clone(),
                memory: zero.clone(),
                prompt_assembly: zero.clone(),
                answer_quality: zero.clone(),
                hallucination: zero.clone(),
                citation_accuracy: zero.clone(),
                grounding: zero,
            },
            claim_verifications: vec![],
            root_causes: vec![FailureRootCause::ExecutionError {
                message: error.to_string(),
            }],
            fix_proposals: vec![],
            trace: trace.clone(),
            is_regression: false,
            is_improvement: false,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Stage 2: Semantic grounding (pure function — no async)
// ──────────────────────────────────────────────────────────────────────────────

fn semantic_grounding_check(
    claim: &str,
    claim_embedding: &[f32],
    chunk_evidence: &[(String, String)],
    chunk_embeddings: &[Vec<f32>],
    memory_evidence: &[(String, String)],
    mem_embeddings: &[Vec<f32>],
) -> ClaimVerification {
    let mut best_score: f32 = 0.0;
    let mut best_source: Option<String> = None;
    let mut best_text: Option<String> = None;

    // Compare against chunks
    for (i, (chunk_id, chunk_text)) in chunk_evidence.iter().enumerate() {
        if let Some(ev_emb) = chunk_embeddings.get(i) {
            let sim = cosine_similarity(claim_embedding, ev_emb);
            if sim > best_score {
                best_score = sim;
                best_source = Some(chunk_id.clone());
                best_text = Some(chunk_text[..chunk_text.len().min(200)].to_string());
            }
        }
    }

    // Compare against memories
    for (i, (mem_id, mem_text)) in memory_evidence.iter().enumerate() {
        if let Some(ev_emb) = mem_embeddings.get(i) {
            let sim = cosine_similarity(claim_embedding, ev_emb);
            if sim > best_score {
                best_score = sim;
                best_source = Some(format!("memory:{}", mem_id));
                best_text = Some(mem_text.clone());
            }
        }
    }

    let support = if best_score >= SEMANTIC_SUPPORT_THRESHOLD {
        ClaimSupport::Supported
    } else if best_score <= SEMANTIC_REJECT_THRESHOLD {
        ClaimSupport::Hallucinated // very low similarity — likely fabricated
    } else {
        ClaimSupport::PartiallySupported // borderline → send to LLM judge
    };

    ClaimVerification {
        claim: claim.to_string(),
        support,
        supporting_evidence: best_text,
        evidence_source: best_source,
        determined_by: "semantic_similarity".to_string(),
        similarity_score: Some(best_score),
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Utilities
// ──────────────────────────────────────────────────────────────────────────────

pub(crate) fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

/// Split answer text into individual factual sentences.
pub(crate) fn tokenize_sentences(text: &str) -> Vec<String> {
    // Simple sentence splitter — splits on '. ', '! ', '? ', '\n'
    let mut sentences = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        current.push(ch);
        if matches!(ch, '.' | '!' | '?') {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                sentences.push(trimmed);
            }
            current.clear();
        } else if ch == '\n' {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                sentences.push(trimmed);
            }
            current.clear();
        }
    }
    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        sentences.push(trimmed);
    }

    sentences
        .into_iter()
        .filter(|s| s.split_whitespace().count() >= 5) // ignore very short fragments
        .collect()
}

/// Returns true if a sentence is non-factual (greeting, transition, etc.)
fn is_non_factual_sentence(s: &str) -> bool {
    let lower = s.to_lowercase();
    let non_factual_prefixes = [
        "based on",
        "according to",
        "in summary",
        "to summarize",
        "here is",
        "here are",
        "please note",
        "note that",
        "i hope",
        "let me",
        "i'll",
        "i will",
        "you can",
        "feel free",
    ];
    non_factual_prefixes.iter().any(|p| lower.starts_with(p))
}

/// English stop words to exclude from key-term extraction.
const STOP_WORDS: &[&str] = &[
    "about", "above", "after", "again", "also", "been", "before", "between",
    "could", "does", "doing", "down", "each", "from", "have", "here", "into",
    "more", "most", "note", "once", "only", "other", "over", "same", "should",
    "some", "such", "than", "that", "their", "them", "then", "there", "these",
    "they", "this", "those", "through", "under", "until", "very", "well",
    "were", "what", "when", "where", "which", "while", "will", "with", "would",
    "your",
];
