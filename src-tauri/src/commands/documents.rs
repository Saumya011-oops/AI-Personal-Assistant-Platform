use tauri::State;

use crate::domain::{AssistantResponse, CommandEnvelope, NormalizedDocument, RetrievalResponse};
use crate::services::AppState;
use crate::services::qdrant::QdrantSearchResult;

#[tauri::command]
pub async fn list_documents(
    state: State<'_, AppState>,
    source_kind: Option<String>,
    query: Option<String>,
) -> Result<CommandEnvelope<Vec<NormalizedDocument>>, String> {
    match state
        .database
        .document_repository()
        .list_documents(source_kind, query)
    {
        Ok(items) => Ok(CommandEnvelope::success(items)),
        Err(error) => Ok(CommandEnvelope::error("DOCUMENT_LIST_FAILED", error.to_string())),
    }
}

#[tauri::command]
pub async fn search_documents_semantic(
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
) -> Result<CommandEnvelope<Vec<QdrantSearchResult>>, String> {
    if query.trim().is_empty() {
        return Ok(CommandEnvelope::success(Vec::new()));
    }

    let limit_val = limit.unwrap_or(5);

    // 1. Generate query embedding using Ollama
    let query_embeddings = match state
        .pipeline_service
        .ollama_service()
        .generate_embeddings(&[query.clone()])
        .await
    {
        Ok(embeddings) => embeddings,
        Err(error) => {
            return Ok(CommandEnvelope::error(
                "EMBEDDING_GENERATION_FAILED",
                error.to_string(),
            ))
        }
    };

    let Some(query_vector) = query_embeddings.into_iter().next() else {
        return Ok(CommandEnvelope::error(
            "EMBEDDING_GENERATION_FAILED",
            "No embedding returned for query".to_string(),
        ));
    };

    // 2. Query Qdrant for similar points
    match state
        .pipeline_service
        .qdrant_service()
        .search_similar_points(query_vector, limit_val, None)
        .await
    {
        Ok(results) => Ok(CommandEnvelope::success(results)),
        Err(error) => Ok(CommandEnvelope::error(
            "SEMANTIC_SEARCH_FAILED",
            error.to_string(),
        )),
    }
}

#[tauri::command]
pub async fn retrieve_documents(
    state: State<'_, AppState>,
    query: String,
) -> Result<CommandEnvelope<RetrievalResponse>, String> {
    if query.trim().is_empty() {
        return Ok(CommandEnvelope::error(
            "INVALID_QUERY",
            "Query cannot be empty".to_string(),
        ));
    }

    match state
        .retrieval_service
        .retrieve_documents(&state.database, &query)
        .await
    {
        Ok(response) => Ok(CommandEnvelope::success(response)),
        Err(error) => Ok(CommandEnvelope::error("RETRIEVAL_FAILED", error.to_string())),
    }
}

#[tauri::command]
pub async fn ask_assistant(
    state: State<'_, AppState>,
    query: String,
    conversation_id: Option<String>,
) -> Result<CommandEnvelope<AssistantResponse>, String> {
    if query.trim().is_empty() {
        return Ok(CommandEnvelope::error(
            "INVALID_QUERY",
            "Query cannot be empty".to_string(),
        ));
    }

    let cid = match conversation_id {
        Some(ref id) if !id.trim().is_empty() => id.clone(),
        _ => {
            match state.memory_service.create_chat("New Chat") {
                Ok(id) => id,
                Err(e) => return Ok(CommandEnvelope::error("CREATE_CHAT_FAILED", e.to_string())),
            }
        }
    };

    match state
        .retrieval_service
        .ask_assistant(&state.database, &state.memory_service, &query, &cid, &state.intent_router)
        .await
    {
        Ok(mut response) => {
            response.conversation_id = Some(cid);
            Ok(CommandEnvelope::success(response))
        }
        Err(error) => Ok(CommandEnvelope::error("ASSISTANT_QUERY_FAILED", error.to_string())),
    }
}

#[tauri::command]
pub async fn clear_all_documents(
    state: State<'_, AppState>,
) -> Result<CommandEnvelope<()>, String> {
    // 1. Clear SQLite tables
    if let Err(err) = state.database.document_repository().clear_all_documents() {
        return Ok(CommandEnvelope::error("SQLITE_CLEAR_FAILED", err.to_string()));
    }

    // 2. Clear Qdrant collection
    if let Err(err) = state.pipeline_service.qdrant_service().clear_collection().await {
        return Ok(CommandEnvelope::error("QDRANT_CLEAR_FAILED", err.to_string()));
    }

    if let Err(err) = state.retrieval_service.clear_sparse_index().await {
        return Ok(CommandEnvelope::error("SPARSE_INDEX_CLEAR_FAILED", err.to_string()));
    }

    Ok(CommandEnvelope::success(()))
}

/// Returns a score distribution report from the last 100 RAG telemetry entries.
/// Use this to calibrate confidence thresholds against real reranker logit distributions.
/// The reranker model (ms-marco-MiniLM-L-6-v2) outputs raw logits in [-10, +10].
#[tauri::command]
pub async fn get_rag_performance_report(
    state: State<'_, AppState>,
) -> Result<CommandEnvelope<serde_json::Value>, String> {
    match state.database.document_repository().get_rag_performance_report() {
        Ok(report) => Ok(CommandEnvelope::success(report)),
        Err(err) => Ok(CommandEnvelope::error("RAG_REPORT_FAILED", err.to_string())),
    }
}
