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
    pub fn get_connection(&self) -> Arc<Mutex<Connection>> {
        self.connection.clone()
    }

    pub fn connect(path: &Path) -> Result<Self> {
        let connection = Connection::open(path)?;
        // Enable WAL mode so concurrent readers (eval tools, debug binaries) work
        // alongside the running Tauri app without SQLITE_BUSY errors.
        connection.pragma_update(None, "journal_mode", "WAL")?;
        // Retry for up to 5 seconds on lock contention before returning SQLITE_BUSY.
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
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

        // Phase 3: Memory tables (idempotent via CREATE TABLE IF NOT EXISTS)
        let sql_003 = include_str!("migrations/003_memory.sql");
        connection.execute_batch(sql_003)?;

        // Alter chat_messages to include token_count, retrieved_document_ids, retrieved_memory_ids, and citations safely
        let _ = connection.execute("ALTER TABLE chat_messages ADD COLUMN token_count INTEGER DEFAULT 0", []);
        let _ = connection.execute("ALTER TABLE chat_messages ADD COLUMN retrieved_document_ids TEXT", []);
        let _ = connection.execute("ALTER TABLE chat_messages ADD COLUMN retrieved_memory_ids TEXT", []);
        let _ = connection.execute("ALTER TABLE chat_messages ADD COLUMN citations TEXT", []);

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

    pub fn reset_assistant_data(&self) -> Result<()> {
        let connection = self.connection.lock().expect("db lock poisoned");
        connection.execute("DELETE FROM credentials", [])?;
        connection.execute("DELETE FROM chat_messages", [])?;
        connection.execute("DELETE FROM sync_state", [])?;
        connection.execute("DELETE FROM documents", [])?;
        connection.execute("DELETE FROM chunks", [])?;
        connection.execute("DELETE FROM chunk_fts", [])?;
        connection.execute("DELETE FROM chats", [])?;
        connection.execute("DELETE FROM conversation_summaries", [])?;
        connection.execute("DELETE FROM memories", [])?;
        connection.execute("DELETE FROM document_clusters", [])?;
        connection.execute("DELETE FROM document_graph_edges", [])?;
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
