use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandEnvelope<T: Serialize> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<AppError>,
}

impl<T: Serialize> CommandEnvelope<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(code: &str, message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(AppError {
                code: code.to_string(),
                message,
                details: None,
            }),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub details: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    pub app_version: String,
    pub environment: String,
    pub rust_backend_available: bool,
    pub database_ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub obsidian_vault_path: Option<String>,
    pub preferred_theme: String,
    pub command_palette_enabled: bool,
    pub telemetry_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct UpdateSettingsInput {
    pub obsidian_vault_path: Option<String>,
    pub preferred_theme: Option<String>,
    pub command_palette_enabled: Option<bool>,
    pub telemetry_enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegrationSummary {
    pub key: String,
    pub label: String,
    pub status: String,
    pub last_synced_at: Option<String>,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedDocument {
    pub id: String,
    pub source_kind: String,
    pub source_external_id: String,
    pub title: String,
    pub content_markdown: String,
    pub content_plaintext: String,
    pub path_or_url: Option<String>,
    pub tags: Vec<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub checksum: String,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRun {
    pub id: String,
    pub integration_key: String,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub documents_discovered: i64,
    pub documents_upserted: i64,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleAuthStatus {
    pub connected: bool,
    pub email: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialRecord {
    pub provider: String,
    pub account_identifier: String,
    pub encrypted_token_blob: String,
    pub expires_at: Option<DateTime<Utc>>,
    pub scopes: Vec<String>,
    pub last_refresh_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentChunk {
    pub id: String,
    pub document_id: String,
    pub ordinal: i64,
    pub content: String,
    pub token_count: i64,
    pub embedding_status: String,
    pub created_at: String,
    pub parent_id: Option<String>,
    pub summary: Option<String>,
    pub classification: Option<String>,
    pub metadata_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MetadataDateRange {
    pub from: Option<String>,
    pub to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MetadataFilters {
    pub source: Option<Vec<String>>,
    pub author: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub category: Option<Vec<String>>,
    pub date_range: Option<MetadataDateRange>,
    pub document_type: Option<Vec<String>>,
    pub topic: Option<Vec<String>>,
}

pub fn normalize_document_type(dt: &str) -> String {
    let lower = dt.to_lowercase();
    if lower.contains("outage report") || lower.contains("outage_report") || lower.contains("outage-report") || lower.contains("outages") || lower.contains("service outage") {
        "outage_report".to_string()
    } else if lower.contains("troubleshooting") || lower.contains("troubleshoot") || lower.contains("guide") {
        "troubleshooting".to_string()
    } else if lower.contains("faq") || lower.contains("faqs") {
        "faq".to_string()
    } else if lower.contains("onboarding") || lower.contains("onboard") {
        "onboarding".to_string()
    } else if lower.contains("incident") || lower.contains("incidents") {
        "incident".to_string()
    } else if lower.contains("policy") || lower.contains("policies") || lower.contains("reimbursement") {
        "policy".to_string()
    } else if lower.contains("roadmap") || lower.contains("roadmaps") {
        "roadmap".to_string()
    } else if lower.contains("billing") || lower.contains("invoice") || lower.contains("invoices") {
        "billing".to_string()
    } else if lower.contains("authentication") || lower.contains("auth") || lower.contains("login") || lower.contains("oauth") || lower.contains("token") {
        "authentication".to_string()
    } else {
        "other".to_string()
    }
}

pub fn normalize_topic(t: &str) -> String {
    let lower = t.to_lowercase();
    if lower.contains("high memory usage") || lower.contains("memory") {
        "high memory usage".to_string()
    } else if lower.contains("mfa setup") || lower.contains("mfa") {
        "mfa".to_string()
    } else if lower.contains("login error") || lower.contains("login") {
        "login".to_string()
    } else if lower.contains("billing") || lower.contains("invoice") || lower.contains("payment") {
        "billing".to_string()
    } else if lower.contains("ui lag") || lower.contains("lag") || lower.contains("performance") {
        "ui lag".to_string()
    } else if lower.contains("data loss") || lower.contains("loss") {
        "data loss".to_string()
    } else if lower.contains("password reset") || lower.contains("password") {
        "password reset".to_string()
    } else if lower.contains("connection") || lower.contains("network") {
        "connection".to_string()
    } else if lower.contains("db corruption") || lower.contains("database") {
        "db corruption".to_string()
    } else if lower.contains("agent offline") || lower.contains("offline") {
        "agent offline".to_string()
    } else if lower.contains("sync failure") || lower.contains("sync") {
        "sync failure".to_string()
    } else if lower.contains("new hire checklist") || lower.contains("onboarding") {
        "new hire checklist".to_string()
    } else if lower.contains("system permissions") || lower.contains("permission") || lower.contains("permissions") {
        "system permissions".to_string()
    } else {
        // Fallback: extract keywords from title
        let stop_words: std::collections::HashSet<&str> = ["and", "the", "for", "with", "about", "guide", "flow", "overview"].iter().cloned().collect();
        let words: Vec<&str> = lower.split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() > 3 && !stop_words.contains(w))
            .collect();
        if !words.is_empty() {
            words.join(" ")
        } else {
            lower
        }
    }
}

pub fn extract_document_type(title: &str, doc_id: &str) -> String {
    let lower_title = title.to_lowercase();
    let lower_id = doc_id.to_lowercase();
    
    // Check id first as it is more specific
    let res = normalize_document_type(&lower_id);
    if res != "other" {
        return res;
    }
    normalize_document_type(&lower_title)
}

pub fn extract_topic(title: &str, _doc_id: &str) -> String {
    normalize_topic(title)
}

pub fn enrich_metadata(title: &str, doc_id: &str, metadata: &mut serde_json::Value) {
    if let Some(obj) = metadata.as_object_mut() {
        if !obj.contains_key("document_type") {
            let doc_type = extract_document_type(title, doc_id);
            obj.insert("document_type".to_string(), serde_json::Value::String(doc_type));
        }
        if !obj.contains_key("topic") {
            let topic = extract_topic(title, doc_id);
            obj.insert("topic".to_string(), serde_json::Value::String(topic));
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QueryComplexity {
    Simple,
    Complex,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RetrievalStrategy {
    Dense,
    Sparse,
    Hybrid,
    Faceted,
    Contextual,
    Recursive,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RetrievalMode {
    Production,
    Evaluation,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AnalysisLevel {
    Level0,
    Level1,
    Level2,
    Level3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryAnalysis {
    pub intent: String,
    pub entities: Vec<String>,
    pub metadata_filters: MetadataFilters,
    pub temporal: bool,
    pub complexity: QueryComplexity,
    pub strategy: RetrievalStrategy,
    pub level: AnalysisLevel,
    pub is_local: bool,
    pub bypass_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SparseSearchHit {
    pub chunk_id: String,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantQueryInput {
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfidenceReport {
    pub confidence: String,       // "high" | "medium" | "low"
    pub confidence_score: u32,    // 0-100
    pub reasons: Vec<String>,
    pub status: String,           // "OK" | "EMPTY_RETRIEVAL" | "LOW_CONFIDENCE_RETRIEVAL" | "AMBIGUOUS_RETRIEVAL" | "PARTIAL_RETRIEVAL"
    pub ambiguity_score: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Citation {
    pub source_document: String,
    pub source_type: String,
    pub chunk_id: String,
    pub retrieval_score: Option<f32>,
    pub rerank_score: f32,
    pub section: Option<String>,
    pub evidence: Option<String>,
    pub evidence_level: Option<String>,
    
    pub document_title: String,
    pub evidence_snippet: Option<String>,
    pub source_connector: String,

    // Legacy fields for backward compatibility
    pub source: String,
    pub document_id: String,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantResponse {
    pub answer: String,
    pub citations: Vec<Citation>,
    pub confidence: Option<ConfidenceReport>,
    pub diagnostics: Option<DiagnosticsPayload>,
    pub conversation_id: Option<String>,
    pub memories: Option<Vec<Value>>,
    /// The fully assembled user-facing prompt sent to the LLM.
    /// Populated by the retrieval service during ask_assistant.
    /// Used by the evaluation framework to validate prompt assembly.
    pub assembled_prompt: Option<String>,
}

// ---------------------------------------------------------------------------
// Diagnostics & Recall Evaluation structs
// ---------------------------------------------------------------------------

/// Snapshot of a single chunk used in pre/post rerank diagnostics.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagChunk {
    pub chunk_id: String,
    pub document_title: String,
    pub retrieval_score: f32,
    pub rerank_score: f32,
}

/// Numerical breakdown of every signal that feeds into the confidence score.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfidenceBreakdown {
    pub reranker_top_sigmoid: f32,
    pub avg_top5_sigmoid: f32,
    pub evidence_consistency_score: i32,
    pub document_focus_bonus: i32,
    pub keyword_overlap_score: f32,
    pub title_match_bonus: i32,
    pub retrieval_signal_score: f32,
    pub final_score: u32,
    pub status: String,
}

/// Retrieval recall metrics — separates retrieval failures from confidence failures.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecallMetrics {
    /// Document titles of the top-20 chunks before reranking.
    pub pre_rerank_doc_titles: Vec<String>,
    /// Document titles of the top-10 chunks after reranking.
    pub post_rerank_doc_titles: Vec<String>,
    pub unique_docs_pre_rerank: usize,
    pub unique_docs_post_rerank: usize,
    /// Whether the reranker changed the #1 ranked document.
    pub top_doc_changed: bool,
    pub pre_rerank_top_score: f32,
    pub post_rerank_top_score: f32,
    /// Fraction of factual anchors in the generated answer that can be traced back
    /// to at least one retrieved chunk (0.0 = none traceable, 1.0 = fully grounded).
    pub fact_coverage: f32,
}

/// Full diagnostics payload attached to every AssistantResponse.
/// UI display is deferred — backend always populates this.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsPayload {
    pub strategy: String,
    pub query_expanded: String,
    pub pre_rerank_chunks: Vec<DiagChunk>,
    pub post_rerank_chunks: Vec<DiagChunk>,
    pub confidence_breakdown: ConfidenceBreakdown,
    pub final_status: String,
    pub recall_metrics: RecallMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievedChunk {
    pub chunk_id: String,
    pub document_id: String,
    pub source: String,
    pub document_title: String,
    pub content: String,
    pub score: f32,
    pub retrieval_score: Option<f32>,
    pub dense_score: Option<f32>,
    pub sparse_score: Option<f32>,
    pub fused_score: Option<f32>,
    pub reranker_score: Option<f32>,
    pub final_score: Option<f32>,
    pub ordinal: i64,
    pub path_or_url: Option<String>,
    pub tags: Vec<String>,
    pub author: Option<String>,
    pub category: Option<String>,
    pub created_at: Option<String>,
    pub modified_at: Option<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalResponse {
    pub query: String,
    pub strategy_used: RetrievalStrategy,
    pub analysis: QueryAnalysis,
    pub results: Vec<RetrievedChunk>,
    pub total_results: usize,
    pub confidence: Option<ConfidenceReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkSearchDocument {
    pub chunk_id: String,
    pub document_id: String,
    pub ordinal: i64,
    pub source_kind: String,
    pub title: String,
    pub content: String,
    pub path_or_url: Option<String>,
    pub tags: Vec<String>,
    pub author: Option<String>,
    pub category: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub metadata: Value,
    pub chunk_metadata_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextChunk {
    pub chunk_id: String,
    pub document_id: String,
    pub source: String,
    pub score: f32,
    pub content: String,
    pub metadata: HashMap<String, Value>,
}
