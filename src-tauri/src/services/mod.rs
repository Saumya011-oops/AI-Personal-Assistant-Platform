use std::sync::Arc;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::Utc;
use rand::RngCore;
use serde_json::json;
use sha2::Digest;
use tauri::AppHandle;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::db::Database;
use crate::domain::{NormalizedDocument, SyncRun};
use crate::integrations::google::PendingOAuthState;
use crate::integrations::notion::NotionIntegration;
pub mod chunker;
pub mod context_builder;
pub mod groq;
pub mod ollama;
pub mod sparse;
pub mod query_analyzer;
pub mod qdrant;
pub mod reranker;
pub mod retrieval;
pub mod pipeline;

use crate::integrations::obsidian::ObsidianIntegration;

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub database: Database,
    pub sync_service: Arc<SyncService>,
    pub credential_service: Arc<CredentialService>,
    pub pipeline_service: Arc<pipeline::PipelineService>,
    pub retrieval_service: Arc<retrieval::RetrievalService>,
    pub oauth_pending_state: Arc<Mutex<Option<PendingOAuthState>>>,
    pub app_handle: AppHandle,
}

#[derive(Clone)]
pub struct SyncService;

impl SyncService {
    pub fn new() -> Self {
        Self
    }

    pub async fn run_notion_sync(
        &self,
        database: &Database,
        integration: &NotionIntegration,
        token: &str,
    ) -> Result<SyncRun> {
        let run = self.create_run("notion");
        database.sync_repository().create_run(&run)?;
        database
            .integration_repository()
            .update_status("notion", "syncing", Some("Fetching Notion documents"), None)?;

        match integration.fetch_documents(token).await {
            Ok(documents) => self.finalize_success(database, run, "notion", documents),
            Err(error) => self.finalize_error(database, run, "notion", error.to_string()),
        }
    }

    pub async fn run_obsidian_sync(
        &self,
        database: &Database,
        integration: &ObsidianIntegration,
    ) -> Result<SyncRun> {
        let run = self.create_run("obsidian");
        database.sync_repository().create_run(&run)?;
        database.integration_repository().update_status(
            "obsidian",
            "syncing",
            Some("Scanning local markdown vault"),
            None,
        )?;

        match integration.scan_documents() {
            Ok(documents) => self.finalize_success(database, run, "obsidian", documents),
            Err(error) => self.finalize_error(database, run, "obsidian", error.to_string()),
        }
    }

    fn create_run(&self, key: &str) -> SyncRun {
        SyncRun {
            id: Uuid::new_v4().to_string(),
            integration_key: key.to_string(),
            status: "running".to_string(),
            started_at: Utc::now().to_rfc3339(),
            finished_at: None,
            documents_discovered: 0,
            documents_upserted: 0,
            error_message: None,
        }
    }

    fn finalize_success(
        &self,
        database: &Database,
        mut run: SyncRun,
        integration_key: &str,
        documents: Vec<NormalizedDocument>,
    ) -> Result<SyncRun> {
        run.documents_discovered = documents.len() as i64;
        run.documents_upserted = database.document_repository().upsert_documents(&documents)?;
        run.status = "completed".to_string();
        run.finished_at = Some(Utc::now().to_rfc3339());
        database.sync_repository().update_run(&run)?;
        database.integration_repository().update_status(
            integration_key,
            "connected",
            Some(&format!("{} documents synced", run.documents_upserted)),
            run.finished_at.as_deref(),
        )?;
        Ok(run)
    }

    fn finalize_error(
        &self,
        database: &Database,
        mut run: SyncRun,
        integration_key: &str,
        message: String,
    ) -> Result<SyncRun> {
        run.status = "failed".to_string();
        run.error_message = Some(message.clone());
        run.finished_at = Some(Utc::now().to_rfc3339());
        database.sync_repository().update_run(&run)?;
        database.integration_repository().update_status(
            integration_key,
            "error",
            Some(&message),
            run.finished_at.as_deref(),
        )?;
        Ok(run)
    }
}

#[derive(Clone)]
pub struct CredentialService {
    cipher_key: [u8; 32],
}

impl CredentialService {
    pub fn new(_handle: &AppHandle) -> Result<Self> {
        let config = AppConfig::load()?;
        let mut key = [0_u8; 32];
        let digest = sha2::Sha256::digest(config.app_secret_key.as_bytes());
        key.copy_from_slice(&digest[..32]);
        Ok(Self { cipher_key: key })
    }

    pub fn new_no_handle() -> Result<Self> {
        let config = AppConfig::load()?;
        let mut key = [0_u8; 32];
        let digest = sha2::Sha256::digest(config.app_secret_key.as_bytes());
        key.copy_from_slice(&digest[..32]);
        Ok(Self { cipher_key: key })
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<String> {
        let cipher = Aes256Gcm::new_from_slice(&self.cipher_key)
            .map_err(|_| anyhow!("failed to initialize encryption key"))?;
        let mut nonce_bytes = [0_u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|_| anyhow!("failed to encrypt credential"))?;
        let payload = json!({
            "nonce": STANDARD.encode(nonce_bytes),
            "ciphertext": STANDARD.encode(ciphertext),
        });
        Ok(payload.to_string())
    }

    pub fn decrypt(&self, encrypted_payload: &str) -> Result<String> {
        let payload: serde_json::Value = serde_json::from_str(encrypted_payload)?;
        let nonce_str = payload["nonce"]
            .as_str()
            .ok_or_else(|| anyhow!("missing nonce in encrypted payload"))?;
        let ciphertext_str = payload["ciphertext"]
            .as_str()
            .ok_or_else(|| anyhow!("missing ciphertext in encrypted payload"))?;

        let nonce_bytes = STANDARD.decode(nonce_str)?;
        let ciphertext = STANDARD.decode(ciphertext_str)?;

        let cipher = Aes256Gcm::new_from_slice(&self.cipher_key)
            .map_err(|_| anyhow!("failed to initialize encryption key"))?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        let plaintext_bytes = cipher
            .decrypt(nonce, ciphertext.as_slice())
            .map_err(|_| anyhow!("failed to decrypt credential"))?;

        let plaintext = String::from_utf8(plaintext_bytes)?;
        Ok(plaintext)
    }
}

