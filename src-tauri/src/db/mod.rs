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
