use tauri::State;

use crate::domain::{CommandEnvelope, SyncRun};
use crate::integrations::obsidian::ObsidianIntegration;
use crate::services::AppState;

#[tauri::command]
pub async fn scan_obsidian_vault(
    state: State<'_, AppState>,
) -> Result<CommandEnvelope<SyncRun>, String> {
    let settings = match state.database.settings_repository().get_settings() {
        Ok(settings) => settings,
        Err(error) => {
            return Ok(CommandEnvelope::error(
                "SETTINGS_READ_FAILED",
                error.to_string(),
            ))
        }
    };

    let Some(vault_path) = settings.obsidian_vault_path.clone() else {
        return Ok(CommandEnvelope::error(
            "VAULT_NOT_CONFIGURED",
            "Obsidian vault path is not configured".to_string(),
        ));
    };

    let integration = ObsidianIntegration::new(vault_path);
    match state
        .sync_service
        .run_obsidian_sync(&state.database, &integration)
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
                            tracing::error!("Failed to rebuild topic graph after Obsidian sync: {}", err);
                        }
                    }
                    Err(err) => {
                        tracing::error!("Background embedding pipeline failed after Obsidian sync: {}", err);
                    }
                }
            });
            Ok(CommandEnvelope::success(result))
        }
        Err(error) => Ok(CommandEnvelope::error("OBSIDIAN_SYNC_FAILED", error.to_string())),
    }
}
