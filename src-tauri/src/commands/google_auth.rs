use tauri::State;

use crate::domain::{CommandEnvelope, GoogleAuthStatus};
use crate::integrations::google::GoogleOAuthService;
use crate::services::AppState;

#[tauri::command]
pub async fn connect_google(
    state: State<'_, AppState>,
) -> Result<CommandEnvelope<GoogleAuthStatus>, String> {
    let service = GoogleOAuthService::new(state.config.clone());
    match service.begin_authorization(&state).await {
        Ok(status) => Ok(CommandEnvelope::success(status)),
        Err(error) => Ok(CommandEnvelope::error("GOOGLE_AUTH_START_FAILED", error.to_string())),
    }
}

#[tauri::command]
pub async fn oauth_callback(
    state: State<'_, AppState>,
    code: String,
    state_param: String,
) -> Result<CommandEnvelope<GoogleAuthStatus>, String> {
    let service = GoogleOAuthService::new(state.config.clone());
    match service.finish_authorization(&state, &code, &state_param).await {
        Ok(status) => Ok(CommandEnvelope::success(status)),
        Err(error) => Ok(CommandEnvelope::error(
            "GOOGLE_AUTH_CALLBACK_FAILED",
            error.to_string(),
        )),
    }
}

#[tauri::command]
pub async fn get_google_auth_status(
    state: State<'_, AppState>,
) -> Result<CommandEnvelope<GoogleAuthStatus>, String> {
    let service = GoogleOAuthService::new(state.config.clone());
    match service.get_status(&state).await {
        Ok(status) => Ok(CommandEnvelope::success(status)),
        Err(error) => Ok(CommandEnvelope::error("GOOGLE_AUTH_STATUS_FAILED", error.to_string())),
    }
}
