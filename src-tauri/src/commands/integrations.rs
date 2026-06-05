use chrono::Utc;
use tauri::State;

use crate::domain::{CommandEnvelope, CredentialRecord, IntegrationSummary};
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

#[tauri::command]
pub async fn save_credential(
    state: State<'_, AppState>,
    provider: String,
    token: String,
) -> Result<CommandEnvelope<()>, String> {
    let encrypted_blob = match state.credential_service.encrypt(&token) {
        Ok(blob) => blob,
        Err(e) => return Ok(CommandEnvelope::error("CREDENTIAL_ENCRYPTION_FAILED", e.to_string())),
    };

    let record = CredentialRecord {
        provider: provider.clone(),
        account_identifier: format!("{}-auth", provider),
        encrypted_token_blob: encrypted_blob,
        expires_at: None,
        scopes: vec![],
        last_refresh_at: Some(Utc::now()),
    };

    match state.database.credential_repository().upsert(&record) {
        Ok(_) => {
            // Also update the integration status to 'connected' in the integrations table
            let _ = state.database.integration_repository().update_status(
                &provider,
                "connected",
                Some("Token saved securely"),
                Some(&Utc::now().to_rfc3339()),
            );
            Ok(CommandEnvelope::success(()))
        }
        Err(e) => Ok(CommandEnvelope::error("CREDENTIAL_SAVE_FAILED", e.to_string())),
    }
}

