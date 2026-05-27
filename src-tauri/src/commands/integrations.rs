use tauri::State;

use crate::domain::{CommandEnvelope, IntegrationSummary};
use crate::services::AppState;

#[tauri::command]
pub async fn list_integrations(
    state: State<'_, AppState>,
) -> Result<CommandEnvelope<Vec<IntegrationSummary>>, String> {
    match state.database.integration_repository().list_integrations() {
        Ok(items) => Ok(CommandEnvelope::success(items)),
        Err(error) => Ok(CommandEnvelope::error("INTEGRATIONS_READ_FAILED", error.to_string())),
    }
}
