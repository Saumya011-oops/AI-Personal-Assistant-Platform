use tauri::State;

use crate::domain::{CommandEnvelope, NormalizedDocument};
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
        .search_similar_points(query_vector, limit_val)
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

    Ok(CommandEnvelope::success(()))
}
