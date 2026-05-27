use tauri::State;

use crate::domain::{AppStatus, CommandEnvelope};
use crate::services::AppState;

#[tauri::command]
pub async fn get_app_status(
    state: State<'_, AppState>,
) -> Result<CommandEnvelope<AppStatus>, String> {
    Ok(CommandEnvelope::success(AppStatus {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        environment: state.config.app_env.clone(),
        rust_backend_available: true,
        database_ready: state.database.health_check().is_ok(),
    }))
}
