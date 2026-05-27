use tauri::State;

use crate::domain::{AppSettings, CommandEnvelope, UpdateSettingsInput};
use crate::services::AppState;

#[tauri::command]
pub async fn get_settings(
    state: State<'_, AppState>,
) -> Result<CommandEnvelope<AppSettings>, String> {
    match state.database.settings_repository().get_settings() {
        Ok(settings) => Ok(CommandEnvelope::success(settings)),
        Err(error) => Ok(CommandEnvelope::error("SETTINGS_READ_FAILED", error.to_string())),
    }
}

#[tauri::command]
pub async fn update_settings(
    state: State<'_, AppState>,
    preferred_theme: Option<String>,
    obsidian_vault_path: Option<String>,
    command_palette_enabled: Option<bool>,
    telemetry_enabled: Option<bool>,
) -> Result<CommandEnvelope<AppSettings>, String> {
    let input = UpdateSettingsInput {
        preferred_theme,
        obsidian_vault_path,
        command_palette_enabled,
        telemetry_enabled,
    };

    match state.database.settings_repository().update_settings(input) {
        Ok(settings) => Ok(CommandEnvelope::success(settings)),
        Err(error) => Ok(CommandEnvelope::error("SETTINGS_UPDATE_FAILED", error.to_string())),
    }
}

#[tauri::command]
pub async fn select_obsidian_vault(
    state: State<'_, AppState>,
    path: String,
) -> Result<CommandEnvelope<AppSettings>, String> {
    let input = UpdateSettingsInput {
        preferred_theme: None,
        obsidian_vault_path: Some(path),
        command_palette_enabled: None,
        telemetry_enabled: None,
    };

    match state.database.settings_repository().update_settings(input) {
        Ok(settings) => Ok(CommandEnvelope::success(settings)),
        Err(error) => Ok(CommandEnvelope::error("VAULT_SAVE_FAILED", error.to_string())),
    }
}
