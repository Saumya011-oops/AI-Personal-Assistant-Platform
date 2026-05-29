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
        
        // 1. Run the initial batch first to set up basic tables
        connection.execute_batch(sql)?;

        // 2. Programmatically upgrade existing chunks table if it lacks Week 4 schema fields
        let mut has_chunk_level = false;
        let mut has_parent_chunk_id = false;
        {
            let mut stmt = connection.prepare("PRAGMA table_info(chunks)")?;
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                let name: String = row.get(1)?;
                if name == "chunk_level" {
                    has_chunk_level = true;
                } else if name == "parent_chunk_id" {
                    has_parent_chunk_id = true;
                }
            }
        }

        if !has_chunk_level {
            tracing::info!("Upgrading database schema: adding chunk_level to chunks table...");
            connection.execute(
                "ALTER TABLE chunks ADD COLUMN chunk_level TEXT NOT NULL DEFAULT 'standard'",
                [],
            )?;
        }

        if !has_parent_chunk_id {
            tracing::info!("Upgrading database schema: adding parent_chunk_id to chunks table...");
            connection.execute(
                "ALTER TABLE chunks ADD COLUMN parent_chunk_id TEXT",
                [],
            )?;
        }

        // 3. Ensure chunks_fts virtual table exists
        connection.execute(
            "CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
              content,
              chunk_id UNINDEXED,
              document_id UNINDEXED,
              tokenize='porter unicode61'
            )",
            [],
        )?;

        // 4. Ensure FTS triggers exist
        connection.execute_batch(
            "CREATE TRIGGER IF NOT EXISTS chunks_fts_ai AFTER INSERT ON chunks BEGIN
              INSERT INTO chunks_fts(content, chunk_id, document_id)
              VALUES (new.content, new.id, new.document_id);
            END;

            CREATE TRIGGER IF NOT EXISTS chunks_fts_ad AFTER DELETE ON chunks BEGIN
              DELETE FROM chunks_fts WHERE chunk_id = old.id;
            END;

            CREATE TRIGGER IF NOT EXISTS chunks_fts_au AFTER UPDATE ON chunks BEGIN
              DELETE FROM chunks_fts WHERE chunk_id = old.id;
              INSERT INTO chunks_fts(content, chunk_id, document_id)
              VALUES (new.content, new.id, new.document_id);
            END;",
        )?;

        // 5. Populate chunks_fts if it is empty but chunks has data
        let fts_count: i64 = connection.query_row("SELECT count(*) FROM chunks_fts", [], |r| r.get(0)).unwrap_or(0);
        let chunks_count: i64 = connection.query_row("SELECT count(*) FROM chunks", [], |r| r.get(0)).unwrap_or(0);
        if fts_count == 0 && chunks_count > 0 {
            tracing::info!("Populating chunks_fts index with existing chunks...");
            connection.execute(
                "INSERT OR IGNORE INTO chunks_fts(content, chunk_id, document_id)
                 SELECT content, id, document_id FROM chunks",
                [],
            )?;
        }

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
