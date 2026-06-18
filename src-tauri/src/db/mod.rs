pub mod repositories;

use std::path::Path;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use rusqlite::Connection;

use self::repositories::{
    credential_repository::CredentialRepository, document_repository::DocumentRepository,
    integration_repository::IntegrationRepository, settings_repository::SettingsRepository,
    sync_repository::SyncRepository,
};

#[derive(Clone)]
pub struct Database {
    connection: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn connect(path: &Path) -> Result<Self> {
        let connection = Connection::open(path)?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    pub fn health_check(&self) -> Result<()> {
        let connection = self.connection.lock().expect("db lock poisoned");
        let _: i32 = connection.query_row("SELECT 1", [], |row| row.get(0))?;
        Ok(())
    }

    pub fn run_migrations(&self) -> Result<()> {
        let sql = include_str!("migrations/001_initial.sql");
        let connection = self.connection.lock().expect("db lock poisoned");
        connection.execute_batch(sql)?;

        let _ = connection.execute("ALTER TABLE chunks ADD COLUMN parent_id TEXT", []);
        let _ = connection.execute("ALTER TABLE chunks ADD COLUMN summary TEXT", []);
        let _ = connection.execute("ALTER TABLE chunks ADD COLUMN classification TEXT", []);
        let _ = connection.execute("ALTER TABLE chunks ADD COLUMN metadata_json TEXT DEFAULT '{}'", []);
        
        let _ = connection.execute("CREATE TABLE IF NOT EXISTS RAG_telemetry (
            id TEXT PRIMARY KEY,
            query TEXT NOT NULL,
            strategy TEXT NOT NULL,
            confidence_score INTEGER NOT NULL,
            status TEXT NOT NULL,
            reasons_json TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )", []);
        
        let _ = connection.execute("ALTER TABLE RAG_telemetry ADD COLUMN confidence TEXT", []);
        let _ = connection.execute("ALTER TABLE RAG_telemetry ADD COLUMN top_document TEXT", []);
        let _ = connection.execute("ALTER TABLE RAG_telemetry ADD COLUMN top_document_score REAL", []);
        let _ = connection.execute("ALTER TABLE RAG_telemetry ADD COLUMN ambiguity_score REAL", []);
        let _ = connection.execute("ALTER TABLE RAG_telemetry ADD COLUMN hop_count INTEGER", []);
        let _ = connection.execute("ALTER TABLE RAG_telemetry ADD COLUMN retrieval_latency_ms INTEGER", []);
        let _ = connection.execute("ALTER TABLE RAG_telemetry ADD COLUMN rerank_latency_ms INTEGER", []);
        let _ = connection.execute("ALTER TABLE RAG_telemetry ADD COLUMN lineage_json TEXT", []);
        // Recall evaluation columns (backward-compatible, added via ALTER TABLE)
        let _ = connection.execute("ALTER TABLE RAG_telemetry ADD COLUMN pre_rerank_top_doc TEXT", []);
        let _ = connection.execute("ALTER TABLE RAG_telemetry ADD COLUMN post_rerank_top_doc TEXT", []);
        let _ = connection.execute("ALTER TABLE RAG_telemetry ADD COLUMN unique_docs_pre INTEGER", []);
        let _ = connection.execute("ALTER TABLE RAG_telemetry ADD COLUMN unique_docs_post INTEGER", []);
        let _ = connection.execute("ALTER TABLE RAG_telemetry ADD COLUMN top_doc_changed INTEGER", []);
        let _ = connection.execute("ALTER TABLE RAG_telemetry ADD COLUMN fact_coverage REAL", []);

        // Phase 2: Topic Graph tables (idempotent via CREATE TABLE IF NOT EXISTS)
        let sql_002 = include_str!("migrations/002_topic_graph.sql");
        connection.execute_batch(sql_002)?;

        Ok(())
    }

    pub fn logout_reset(&self) -> Result<()> {
        let connection = self.connection.lock().expect("db lock poisoned");
        connection.execute("DELETE FROM credentials", [])?;
        connection.execute("DELETE FROM chat_messages", [])?;
        connection.execute("DELETE FROM sync_state", [])?;
        connection.execute("DELETE FROM documents", [])?;
        connection.execute("DELETE FROM chunk_fts", [])?;
        connection.execute("UPDATE integrations SET status = 'not_connected', detail = NULL, last_synced_at = NULL", [])?;
        connection.execute("UPDATE settings SET obsidian_vault_path = NULL", [])?;
        Ok(())
    }

    pub fn document_repository(&self) -> DocumentRepository {
        DocumentRepository::new(self.connection.clone())
    }

    pub fn settings_repository(&self) -> SettingsRepository {
        SettingsRepository::new(self.connection.clone())
    }

    pub fn integration_repository(&self) -> IntegrationRepository {
        IntegrationRepository::new(self.connection.clone())
    }

    pub fn sync_repository(&self) -> SyncRepository {
        SyncRepository::new(self.connection.clone())
    }

    pub fn credential_repository(&self) -> CredentialRepository {
        CredentialRepository::new(self.connection.clone())
    }
}
