/// evaluation/types.rs
///
/// Shared types for the QA Evaluation Framework.
/// Every layer (generator, executor, evaluator, regression, reporter) imports from here.

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::domain::{
    Citation, ConfidenceReport, DiagnosticsPayload, QueryAnalysis, RetrievedChunk,
};
use crate::services::memory::{DbMemory, RankedMemory};

// ──────────────────────────────────────────────────────────────────────────────
// Test Identity
// ──────────────────────────────────────────────────────────────────────────────

/// Which top-level suite this test belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TestSuite {
    /// Factual / semantic / keyword / metadata / date / author retrieval.
    Retrieval,
    /// Long-term / short-term / episodic / preference / goal / task memory.
    Memory,
    /// Multi-turn queries spanning both retrieval and memory.
    Combined,
    /// Canary queries for facts that do NOT exist — must not hallucinate.
    Hallucination,
    /// Verify every cited chunk_id exists in the retrieved set.
    Citation,
    /// Verify prompt ordering, dedup, overflow, truncation.
    PromptAssembly,
    /// Per-claim attribution — every statement traceable to evidence.
    Grounding,
    /// Re-runs of previously-passing tests to detect regressions.
    Regression,
}

impl std::fmt::Display for TestSuite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            TestSuite::Retrieval => "Retrieval",
            TestSuite::Memory => "Memory",
            TestSuite::Combined => "Combined",
            TestSuite::Hallucination => "Hallucination",
            TestSuite::Citation => "Citation",
            TestSuite::PromptAssembly => "PromptAssembly",
            TestSuite::Grounding => "Grounding",
            TestSuite::Regression => "Regression",
        };
        write!(f, "{}", s)
    }
}

/// Fine-grained category within a suite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TestCategory {
    // Retrieval categories
    FactualLookup,
    SemanticSearch,
    KeywordLookup,
    MetadataFilter,
    AuthorFilter,
    DateFilter,
    TagFilter,
    BroadQuestion,
    SpecificQuestion,
    ComparisonQuestion,
    RecursiveRetrieval,
    MultiHopReasoning,
    DocumentSummary,
    CodeRelated,
    PolicyQuestion,
    // Edge cases
    TypoQuery,
    SynonymQuery,
    AcronymQuery,
    IncompleteQuery,
    AmbiguousQuery,
    EmptyRetrieval,
    // Memory categories
    LongTermRecall,
    ShortTermRecall,
    EpisodicRecall,
    PreferenceRecall,
    GoalRecall,
    TaskRecall,
    MemoryUpdate,
    MemoryOverwrite,
    MemoryDeduplication,
    MemoryFreshness,
    IrrelevantMemoryRejection,
    StaleMemoryRejection,
    // Hallucination
    HallucinationCanary,
    FabricatedCitation,
    // Prompt
    PromptOrderCheck,
    PromptDuplicationCheck,
    PromptOverflowCheck,
    ContextTruncation,
}

// ──────────────────────────────────────────────────────────────────────────────
// Ground Truth — what the correct answer MUST contain
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroundTruth {
    /// Factual statements the answer MUST support (used for per-claim grounding).
    pub required_facts: Vec<String>,
    /// Document IDs (or partial title substrings) that MUST appear in retrieved chunks.
    pub required_doc_ids: Vec<String>,
    /// Entities that MUST appear in retrieved chunk text.
    pub required_entities: Vec<String>,
    /// Topics that MUST appear in retrieved chunk text.
    pub required_topics: Vec<String>,
    /// Keywords that MUST appear in the final answer.
    pub required_answer_keywords: Vec<String>,
    /// Terms that MUST NOT appear in the final answer (hallucination seeds).
    pub forbidden_terms: Vec<String>,
    /// Memory content substrings that MUST be recalled.
    pub required_memory_content: Vec<String>,
    /// Minimum number of citations the answer must include.
    pub min_citations: usize,
    /// Expected retrieval confidence status values (any one must match).
    pub expected_statuses: Vec<String>,
    /// Expected retrieval confidence level (e.g. "high", "medium", "low").
    pub expected_confidence: Option<String>,
    /// Characteristics the answer should exhibit (used for soft scoring).
    pub answer_characteristics: Vec<AnswerCharacteristic>,
}

impl Default for GroundTruth {
    fn default() -> Self {
        Self {
            required_facts: vec![],
            required_doc_ids: vec![],
            required_entities: vec![],
            required_topics: vec![],
            required_answer_keywords: vec![],
            forbidden_terms: vec![],
            required_memory_content: vec![],
            min_citations: 0,
            expected_statuses: vec!["OK".to_string()],
            expected_confidence: None,
            answer_characteristics: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AnswerCharacteristic {
    /// Answer must contain a comparison between two or more items.
    ContainsComparison,
    /// Answer must acknowledge uncertainty / limited evidence.
    AcknowledgesUncertainty,
    /// Answer must NOT make any definitive factual claim.
    NoClaims,
    /// Answer must contain step-by-step instructions.
    StepByStep,
    /// Answer must cite a specific date or time range.
    ContainsDate,
}

// ──────────────────────────────────────────────────────────────────────────────
// Memory Fixture — for seeded (reproducible) memory tests
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryFixture {
    /// Unique ID to use when inserting — must be recognisable for cleanup.
    pub id: String,
    /// Memory type: "PROFILE" | "PREFERENCE" | "EPISODE" | "GOAL" | "TASK" | "FACT"
    pub memory_type: String,
    /// Content of the memory.
    pub content: String,
    /// Importance score (1-10).
    pub importance: i64,
    /// How old to simulate (days ago) — affects recency scoring.
    pub simulated_age_days: f64,
    /// Whether this memory should be intentionally stale (for staleness tests).
    pub is_stale: bool,
}

// ──────────────────────────────────────────────────────────────────────────────
// Test Case
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestConstraints {
    /// If true, a conversational context (prior messages) is injected.
    pub has_conversation_context: bool,
    /// Prior messages to inject (role, content pairs).
    pub prior_messages: Vec<(String, String)>,
    /// Expected retrieval strategy the system should choose.
    pub expected_strategy: Option<String>,
    /// Maximum acceptable latency in ms.
    pub max_latency_ms: Option<u64>,
    /// Whether this test uses seeded memories (vs. live memories).
    pub uses_seeded_memories: bool,
}

impl Default for TestConstraints {
    fn default() -> Self {
        Self {
            has_conversation_context: false,
            prior_messages: vec![],
            expected_strategy: None,
            max_latency_ms: None,
            uses_seeded_memories: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCase {
    pub id: String,
    pub suite: TestSuite,
    pub category: TestCategory,
    pub description: String,
    pub query: String,
    pub ground_truth: GroundTruth,
    /// Memory fixtures to seed before this test runs (cleaned up after).
    pub memory_fixtures: Vec<MemoryFixture>,
    pub constraints: TestConstraints,
}

// ──────────────────────────────────────────────────────────────────────────────
// Execution Trace — every intermediate stage captured
// ──────────────────────────────────────────────────────────────────────────────

/// Latency breakdown by pipeline stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatencyBreakdown {
    pub query_analysis_ms: u64,
    pub dense_retrieval_ms: u64,
    pub sparse_retrieval_ms: u64,
    pub reranking_ms: u64,
    pub memory_retrieval_ms: u64,
    pub prompt_assembly_ms: u64,
    pub llm_generation_ms: u64,
    pub total_ms: u64,
}

impl Default for LatencyBreakdown {
    fn default() -> Self {
        Self {
            query_analysis_ms: 0,
            dense_retrieval_ms: 0,
            sparse_retrieval_ms: 0,
            reranking_ms: 0,
            memory_retrieval_ms: 0,
            prompt_assembly_ms: 0,
            llm_generation_ms: 0,
            total_ms: 0,
        }
    }
}

/// Full capture of every intermediate step in the pipeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTrace {
    pub test_id: String,
    pub query: String,
    pub query_analysis: Option<QueryAnalysis>,
    pub expanded_query: Option<String>,
    /// Raw dense (Qdrant) results before reranking.
    pub pre_rerank_chunks: Vec<RetrievedChunk>,
    /// Final reranked results used for answer generation.
    pub post_rerank_chunks: Vec<RetrievedChunk>,
    /// Recalled memories (ranked).
    pub recalled_memories: Vec<RankedMemorySnapshot>,
    /// The exact prompt sent to the LLM.
    pub prompt_assembled: String,
    /// Raw LLM output.
    pub llm_response: String,
    /// Parsed citations from the response.
    pub citations: Vec<Citation>,
    /// Final formatted answer.
    pub final_answer: String,
    /// Full confidence report.
    pub confidence: Option<ConfidenceReport>,
    /// Full diagnostics from the retrieval service.
    pub diagnostics: Option<DiagnosticsPayload>,
    pub latency: LatencyBreakdown,
    /// Any error that occurred; trace is still stored for debugging.
    pub error: Option<String>,
}

/// Serializable snapshot of a ranked memory (avoids non-Serialize fields).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankedMemorySnapshot {
    pub memory: DbMemory,
    pub final_score: f64,
    pub similarity: f64,
    pub importance_score: f64,
    pub recency_score: f64,
    pub access_freq_score: f64,
}

impl From<RankedMemory> for RankedMemorySnapshot {
    fn from(rm: RankedMemory) -> Self {
        Self {
            memory: rm.memory,
            final_score: rm.final_score,
            similarity: rm.similarity,
            importance_score: rm.importance_score,
            recency_score: rm.recency_score,
            access_freq_score: rm.access_freq_score,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Claim-Level Grounding
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClaimSupport {
    /// Claim is directly supported by a retrieved chunk or recalled memory.
    Supported,
    /// Claim is partially supported — evidence is present but incomplete.
    PartiallySupported,
    /// Claim is not traceable to any evidence but may be a valid inference.
    Unsupported,
    /// Claim directly contradicts or fabricates information not in evidence.
    Hallucinated,
}

impl std::fmt::Display for ClaimSupport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClaimSupport::Supported => write!(f, "SUPPORTED"),
            ClaimSupport::PartiallySupported => write!(f, "PARTIALLY_SUPPORTED"),
            ClaimSupport::Unsupported => write!(f, "UNSUPPORTED"),
            ClaimSupport::Hallucinated => write!(f, "HALLUCINATED"),
        }
    }
}

/// Result of analysing a single factual sentence in the answer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimVerification {
    /// The original sentence extracted from the answer.
    pub claim: String,
    pub support: ClaimSupport,
    /// The evidence snippet that supports/partially supports the claim (if any).
    pub supporting_evidence: Option<String>,
    /// Source (chunk_id or memory_id) of the evidence.
    pub evidence_source: Option<String>,
    /// Stage that determined the verdict:
    /// "exact_match" | "semantic_similarity" | "llm_judge" | "no_evidence"
    pub determined_by: String,
    /// Similarity score if determined by semantic check.
    pub similarity_score: Option<f32>,
}

// ──────────────────────────────────────────────────────────────────────────────
// Dimension Scores
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DimensionScore {
    pub score: f32,        // 0.0–100.0
    pub passed: bool,      // score >= threshold
    pub threshold: f32,
    pub details: Vec<String>, // human-readable breakdown
}

impl DimensionScore {
    pub fn new(score: f32, threshold: f32, details: Vec<String>) -> Self {
        Self {
            passed: score >= threshold,
            score,
            threshold,
            details,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalScorecard {
    pub retrieval: DimensionScore,
    pub memory: DimensionScore,
    pub prompt_assembly: DimensionScore,
    pub answer_quality: DimensionScore,
    pub hallucination: DimensionScore, // 100 = 0 hallucinations (inverted)
    pub citation_accuracy: DimensionScore,
    pub grounding: DimensionScore,
}

impl EvalScorecard {
    pub fn production_ready(&self) -> bool {
        self.retrieval.passed
            && self.memory.passed
            && self.prompt_assembly.passed
            && self.answer_quality.passed
            && self.hallucination.passed
            && self.citation_accuracy.passed
            && self.grounding.passed
    }

    pub fn overall_score(&self) -> f32 {
        let scores = [
            self.retrieval.score,
            self.memory.score,
            self.prompt_assembly.score,
            self.answer_quality.score,
            self.hallucination.score,
            self.citation_accuracy.score,
            self.grounding.score,
        ];
        scores.iter().sum::<f32>() / scores.len() as f32
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Per-Test Evaluation Result
// ──────────────────────────────────────────────────────────────────────────────

/// Possible root causes of a failure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FailureRootCause {
    /// No documents matched the query.
    EmptyRetrieval,
    /// Wrong documents retrieved (ranking issue).
    RankingFailure { expected: Vec<String>, got: Vec<String> },
    /// Correct documents retrieved but key entities missing from chunks.
    EntityRecallGap { missing: Vec<String> },
    /// Memory recalled incorrectly or stale memory surfaced.
    MemoryRecallError { detail: String },
    /// Prompt missing required section or has wrong ordering.
    PromptAssemblyError { detail: String },
    /// Answer contains claim with no evidence support.
    HallucinatedClaim { claim: String },
    /// Citation references a chunk_id not in retrieved set.
    FabricatedCitation { chunk_id: String },
    /// Answer is incomplete or missing required keywords.
    AnswerQualityFailure { missing: Vec<String> },
    /// Execution error (service unavailable, timeout, etc.).
    ExecutionError { message: String },
    /// Unknown cause requiring manual investigation.
    Unknown { detail: String },
}

/// Fix proposal (never auto-applied to production logic without user approval).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixProposal {
    pub file: String,
    pub function: Option<String>,
    pub line_hint: Option<u32>,
    pub description: String,
    /// Whether this fix is safe to auto-apply (only for mechanical issues).
    pub auto_applicable: bool,
    /// The exact diff or code change suggested.
    pub proposed_change: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalResult {
    pub test_id: String,
    pub suite: TestSuite,
    pub category: TestCategory,
    pub query: String,
    pub passed: bool,
    pub scorecard: EvalScorecard,
    pub claim_verifications: Vec<ClaimVerification>,
    pub root_causes: Vec<FailureRootCause>,
    pub fix_proposals: Vec<FixProposal>,
    pub trace: ExecutionTrace,
    /// Whether this is a regression vs. the baseline.
    pub is_regression: bool,
    /// Whether this is an improvement vs. the baseline.
    pub is_improvement: bool,
}

// ──────────────────────────────────────────────────────────────────────────────
// Regression Baseline
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineEntry {
    pub test_id: String,
    pub passed: bool,
    pub overall_score: f32,
    pub retrieval_score: f32,
    pub memory_score: f32,
    pub citation_score: f32,
    pub grounding_score: f32,
    pub hallucination_score: f32,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Baseline {
    pub entries: HashMap<String, BaselineEntry>,
    pub created_at: String,
    pub framework_version: String,
}

// ──────────────────────────────────────────────────────────────────────────────
// Final Run Report
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunReport {
    pub run_id: String,
    pub timestamp: String,
    pub production_ready: bool,
    pub total_tests: usize,
    pub passed_tests: usize,
    pub failed_tests: usize,
    pub regressions: Vec<String>,
    pub improvements: Vec<String>,
    pub overall_scorecard: EvalScorecard,
    pub per_suite_scores: HashMap<String, f32>,
    pub results: Vec<EvalResult>,
}
