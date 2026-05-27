use std::sync::{Arc, Mutex};

use anyhow::Result;
use rusqlite::{params, Connection};

use crate::domain::SyncRun;

#[derive(Clone)]
pub struct SyncRepository {
    connection: Arc<Mutex<Connection>>,
}

impl SyncRepository {
    pub fn new(connection: Arc<Mutex<Connection>>) -> Self {
        Self { connection }
    }

    pub fn create_run(&self, run: &SyncRun) -> Result<()> {
        let connection = self.connection.lock().expect("db lock poisoned");
        connection.execute(
            "INSERT INTO sync_state (
              id, integration_key, status, started_at, finished_at, documents_discovered, documents_upserted, error_message
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                run.id,
                run.integration_key,
                run.status,
                run.started_at,
                run.finished_at,
                run.documents_discovered,
                run.documents_upserted,
                run.error_message
            ],
        )?;
        Ok(())
    }

    pub fn update_run(&self, run: &SyncRun) -> Result<()> {
        let connection = self.connection.lock().expect("db lock poisoned");
        connection.execute(
            "UPDATE sync_state SET status = ?2, finished_at = ?3, documents_discovered = ?4, documents_upserted = ?5, error_message = ?6 WHERE id = ?1",
            params![
                run.id,
                run.status,
                run.finished_at,
                run.documents_discovered,
                run.documents_upserted,
                run.error_message
            ],
        )?;
        Ok(())
    }
}
