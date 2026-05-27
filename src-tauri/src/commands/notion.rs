use tauri::State;

use crate::domain::{CommandEnvelope, SyncRun};
use crate::integrations::notion::NotionIntegration;
use crate::services::AppState;

#[tauri::command]
pub async fn sync_notion_documents(
    state: State<'_, AppState>,
    _cursor: Option<String>,
) -> Result<CommandEnvelope<SyncRun>, String> {
    let integration = NotionIntegration::new(state.config.clone());
    match state
        .sync_service
        .run_notion_sync(&state.database, &integration)
        .await
    {
        Ok(result) => {
            let pipeline = state.pipeline_service.clone();
            let db = state.database.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(err) = pipeline.process_all_pending_documents(&db).await {
                    tracing::error!("Background embedding pipeline failed after Notion sync: {}", err);
                }
            });
            Ok(CommandEnvelope::success(result))
        }
        Err(error) => Ok(CommandEnvelope::error("NOTION_SYNC_FAILED", error.to_string())),
    }
}
