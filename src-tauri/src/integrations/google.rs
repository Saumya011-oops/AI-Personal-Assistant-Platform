use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{Duration, Utc};
use rand::{distributions::Alphanumeric, Rng};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::State;
use tauri_plugin_shell::ShellExt;

use crate::config::AppConfig;
use crate::domain::{CredentialRecord, GoogleAuthStatus};
use crate::services::AppState;

#[derive(Clone)]
pub struct GoogleOAuthService {
    config: AppConfig,
    client: Client,
}

#[derive(Clone)]
pub struct PendingOAuthState {
    pub state: String,
    pub verifier: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct GoogleTokenResponse {
    access_token: String,
    expires_in: i64,
    refresh_token: Option<String>,
    scope: String,
    token_type: String,
    id_token: Option<String>,
}

impl GoogleOAuthService {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }

    pub async fn begin_authorization(&self, state: &State<'_, AppState>) -> Result<GoogleAuthStatus> {
        let client_id = self
            .config
            .google_client_id
            .clone()
            .ok_or_else(|| anyhow!("GOOGLE_CLIENT_ID is not configured"))?;

        let verifier: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(64)
            .map(char::from)
            .collect();
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        let oauth_state: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();

        {
            let mut pending = state.oauth_pending_state.lock().await;
            *pending = Some(PendingOAuthState {
                state: oauth_state.clone(),
                verifier,
            });
        }

        let url = url::Url::parse_with_params(
            "https://accounts.google.com/o/oauth2/v2/auth",
            &[
                ("client_id", client_id.as_str()),
                ("redirect_uri", self.config.google_redirect_uri.as_str()),
                ("response_type", "code"),
                ("scope", self.config.google_auth_scopes.as_str()),
                ("access_type", "offline"),
                ("prompt", "consent"),
                ("state", oauth_state.as_str()),
                ("code_challenge", challenge.as_str()),
                ("code_challenge_method", "S256"),
            ],
        )?;

        state.app_handle.shell().open(url.to_string(), None)?;

        Ok(GoogleAuthStatus {
            connected: false,
            email: None,
            expires_at: None,
        })
    }

    pub async fn finish_authorization(
        &self,
        state: &State<'_, AppState>,
        code: &str,
        state_param: &str,
    ) -> Result<GoogleAuthStatus> {
        let pending = {
            let mut pending = state.oauth_pending_state.lock().await;
            pending.take()
        }
        .ok_or_else(|| anyhow!("No pending OAuth session"))?;

        if pending.state != state_param {
            return Err(anyhow!("OAuth state mismatch"));
        }

        let client_id = self
            .config
            .google_client_id
            .clone()
            .ok_or_else(|| anyhow!("GOOGLE_CLIENT_ID is not configured"))?;
        let client_secret = self
            .config
            .google_client_secret
            .clone()
            .ok_or_else(|| anyhow!("GOOGLE_CLIENT_SECRET is not configured"))?;

        let token: GoogleTokenResponse = self
            .client
            .post("https://oauth2.googleapis.com/token")
            .form(&[
                ("code", code),
                ("client_id", client_id.as_str()),
                ("client_secret", client_secret.as_str()),
                ("redirect_uri", self.config.google_redirect_uri.as_str()),
                ("grant_type", "authorization_code"),
                ("code_verifier", pending.verifier.as_str()),
            ])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let encrypted_blob = state
            .credential_service
            .encrypt(&serde_json::to_string(&token)?)?;
        let expires_at = Utc::now() + Duration::seconds(token.expires_in);

        state.database.credential_repository().upsert(&CredentialRecord {
            provider: "google".to_string(),
            account_identifier: "google-auth".to_string(),
            encrypted_token_blob: encrypted_blob,
            expires_at: Some(expires_at),
            scopes: token
                .scope
                .split_whitespace()
                .map(ToString::to_string)
                .collect(),
            last_refresh_at: Some(Utc::now()),
        })?;

        state.database.integration_repository().update_status(
            "google",
            "connected",
            Some("OAuth session established"),
            Some(&Utc::now().to_rfc3339()),
        )?;

        Ok(GoogleAuthStatus {
            connected: true,
            email: None,
            expires_at: Some(expires_at.to_rfc3339()),
        })
    }

    pub async fn get_status(&self, state: &State<'_, AppState>) -> Result<GoogleAuthStatus> {
        let credential = state.database.credential_repository().get_by_provider("google")?;

        if let Some(record) = credential {
            return Ok(GoogleAuthStatus {
                connected: true,
                email: Some(record.account_identifier),
                expires_at: record.expires_at.map(|value| value.to_rfc3339()),
            });
        }

        Ok(GoogleAuthStatus {
            connected: false,
            email: None,
            expires_at: None,
        })
    }
}
