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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryAnalysis {
    pub intent: String,
    pub entities: Vec<String>,
    pub metadata_filters: MetadataFilters,
    pub temporal: bool,
    pub complexity: QueryComplexity,
    pub strategy: RetrievalStrategy,
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
pub struct Citation {
    pub source: String,
    pub document_id: String,
    pub chunk_id: String,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantResponse {
    pub answer: String,
    pub citations: Vec<Citation>,
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
