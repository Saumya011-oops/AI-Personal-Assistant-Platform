use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
    /// 'standard' | 'parent' | 'child'
    pub chunk_level: String,
    /// Non-null for child chunks; references the parent summary chunk id
    pub parent_chunk_id: Option<String>,
    pub created_at: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Week 4 — Retrieval Layer Types
// ─────────────────────────────────────────────────────────────────────────────

/// Input to the unified retrieve_documents command.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalRequest {
    pub query: String,
    /// One of: "dense" | "sparse" | "hybrid" | "faceted" | "contextual" | "recursive"
    pub strategy: String,
    pub limit: Option<usize>,
    pub filters: Option<RetrievalFilters>,
    /// Number of sibling chunks to include on each side (for contextual strategy)
    pub context_window: Option<usize>,
}

/// Optional payload filters applied during faceted or other filtered searches.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalFilters {
    pub source_kind: Option<String>,
    pub tags: Option<Vec<String>>,
    pub date_after: Option<String>,
    pub date_before: Option<String>,
}

/// A single context chunk surrounding a primary result (contextual strategy).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextChunk {
    pub ordinal: i64,
    pub content: String,
    pub is_primary: bool,
}

/// A single retrieval result, enriched with context and parent information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalResult {
    pub chunk_id: String,
    pub document_id: String,
    pub document_title: String,
    pub source_kind: String,
    pub content: String,
    /// Normalised similarity / relevance score in [0, 1] range where applicable
    pub score: f64,
    /// 1-indexed rank within this strategy's result list
    pub rank: usize,
    pub strategy: String,
    /// Surrounding chunks (populated by contextual and recursive strategies)
    pub context_chunks: Vec<ContextChunk>,
    /// Parent summary chunk content (populated by recursive strategy)
    pub parent_content: Option<String>,
    pub path_or_url: Option<String>,
    pub tags: Vec<String>,
}

/// Top-level response returned by retrieve_documents.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalResponse {
    pub results: Vec<RetrievalResult>,
    pub strategy_used: String,
    pub total_results: usize,
    pub query: String,
    pub latency_ms: u64,
}

/// Used by test_retrieval_strategies — runs all 6 strategies and returns each.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AllStrategiesResult {
    pub query: String,
    pub dense: RetrievalResponse,
    pub sparse: RetrievalResponse,
    pub hybrid: RetrievalResponse,
    pub faceted: RetrievalResponse,
    pub contextual: RetrievalResponse,
    pub recursive: RetrievalResponse,
    pub total_latency_ms: u64,
}
