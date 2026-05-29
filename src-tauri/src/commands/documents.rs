use tauri::State;

use crate::domain::{
    AllStrategiesResult, CommandEnvelope, NormalizedDocument, RetrievalRequest, RetrievalResponse,
};
use crate::services::AppState;
use crate::services::qdrant::QdrantSearchResult;
use crate::services::retrieval::RetrievalService;

// ─────────────────────────────────────────────────────────────────────────────
// Existing commands (unchanged)
// ─────────────────────────────────────────────────────────────────────────────

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

// ─────────────────────────────────────────────────────────────────────────────
// Week 4 — New retrieval commands
// ─────────────────────────────────────────────────────────────────────────────

/// Primary retrieval command. Dispatches to one of the six strategies based on
/// `request.strategy`: "dense" | "sparse" | "hybrid" | "faceted" | "contextual" | "recursive"
#[tauri::command]
pub async fn retrieve_documents(
    state: State<'_, AppState>,
    query: String,
    strategy: String,
    limit: Option<usize>,
    filters: Option<crate::domain::RetrievalFilters>,
    context_window: Option<usize>,
) -> Result<CommandEnvelope<RetrievalResponse>, String> {
    if query.trim().is_empty() {
        return Ok(CommandEnvelope::success(RetrievalResponse {
            results: Vec::new(),
            strategy_used: strategy,
            total_results: 0,
            query,
            latency_ms: 0,
        }));
    }

    let request = RetrievalRequest {
        query,
        strategy,
        limit,
        filters,
        context_window,
    };

    match RetrievalService::retrieve(
        &request,
        &state.database,
        state.pipeline_service.ollama_service(),
        state.pipeline_service.qdrant_service(),
    )
    .await
    {
        Ok(response) => Ok(CommandEnvelope::success(response)),
        Err(err) => Ok(CommandEnvelope::error("RETRIEVAL_FAILED", err.to_string())),
    }
}

/// Runs all six retrieval strategies for the same query and returns their results
/// side-by-side. Useful for comparison mode in the UI and for the exit-criteria
/// test suite validation.
#[tauri::command]
pub async fn test_retrieval_strategies(
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
) -> Result<CommandEnvelope<AllStrategiesResult>, String> {
    if query.trim().is_empty() {
        return Ok(CommandEnvelope::error(
            "EMPTY_QUERY",
            "Query must not be empty for strategy comparison".to_string(),
        ));
    }

    let limit_val = limit.unwrap_or(5);

    match RetrievalService::retrieve_all_strategies(
        &query,
        limit_val,
        &state.database,
        state.pipeline_service.ollama_service(),
        state.pipeline_service.qdrant_service(),
    )
    .await
    {
        Ok(result) => Ok(CommandEnvelope::success(result)),
        Err(err) => Ok(CommandEnvelope::error("STRATEGY_TEST_FAILED", err.to_string())),
    }
}

/// Rebuilds the recursive (parent+child) index for all documents.
/// This is called from the UI "Rebuild Index" button.
#[tauri::command]
pub async fn rebuild_recursive_index(
    state: State<'_, AppState>,
) -> Result<CommandEnvelope<usize>, String> {
    match state
        .pipeline_service
        .rebuild_recursive_index(&state.database)
        .await
    {
        Ok(count) => Ok(CommandEnvelope::success(count)),
        Err(err) => Ok(CommandEnvelope::error("REBUILD_INDEX_FAILED", err.to_string())),
    }
}
