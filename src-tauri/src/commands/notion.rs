use tauri::State;

use crate::domain::{CommandEnvelope, SyncRun};
use crate::integrations::notion::NotionIntegration;
use crate::services::AppState;

#[tauri::command]
pub async fn sync_notion_documents(
    state: State<'_, AppState>,
    _cursor: Option<String>,
) -> Result<CommandEnvelope<SyncRun>, String> {
    let db_token = match state.database.credential_repository().get_by_provider("notion") {
        Ok(Some(record)) => {
            match state.credential_service.decrypt(&record.encrypted_token_blob) {
                Ok(token) => Some(token),
                Err(err) => {
                    tracing::error!("Failed to decrypt Notion token: {}", err);
                    None
                }
            }
        }
        _ => None,
    };

    let token = match db_token {
        Some(t) => t,
        None => return Ok(CommandEnvelope::error("NOTION_TOKEN_MISSING", "Notion token is not configured. Please set it in onboarding or settings.".to_string())),
    };

    let integration = NotionIntegration::new(state.config.clone());
    match state
        .sync_service
        .run_notion_sync(&state.database, &integration, &token)
        .await
    {
        Ok(result) => {
            let pipeline = state.pipeline_service.clone();
            let retrieval = state.retrieval_service.clone();
            let db = state.database.clone();
            tauri::async_runtime::spawn(async move {
                match pipeline.process_all_pending_documents(&db).await {
                    Ok(_) => {
                        if let Err(err) = retrieval.rebuild_topic_graph(&db).await {
                            tracing::error!("Failed to rebuild topic graph after Notion sync: {}", err);
                        }
                    }
                    Err(err) => {
                        tracing::error!("Background embedding pipeline failed after Notion sync: {}", err);
                    }
                }
            });
            Ok(CommandEnvelope::success(result))
        }
        Err(error) => Ok(CommandEnvelope::error("NOTION_SYNC_FAILED", error.to_string())),
    }
}
