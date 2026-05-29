use std::sync::{Arc, Mutex};

use anyhow::Result;
use rusqlite::{params, Connection};

use crate::domain::IntegrationSummary;

#[derive(Clone)]
pub struct IntegrationRepository {
    connection: Arc<Mutex<Connection>>,
}

impl IntegrationRepository {
    pub fn new(connection: Arc<Mutex<Connection>>) -> Self {
        let repository = Self { connection };
        repository.seed_defaults().ok();
        repository
    }

    fn seed_defaults(&self) -> Result<()> {
        let connection = self.connection.lock().expect("db lock poisoned");
        for (key, label) in [
            ("notion", "Notion"),
            ("obsidian", "Obsidian"),
            ("google", "Google OAuth"),
        ] {
            connection.execute(
                "INSERT OR IGNORE INTO integrations (key, label) VALUES (?1, ?2)",
                params![key, label],
            )?;
        }
        Ok(())
    }

    pub fn list_integrations(&self) -> Result<Vec<IntegrationSummary>> {
        let connection = self.connection.lock().expect("db lock poisoned");
        let mut statement = connection.prepare(
            "SELECT \
                i.key, \
                i.label, \
                CASE \
                    WHEN i.key = 'google' THEN \
                        CASE \
                            WHEN EXISTS (SELECT 1 FROM credentials WHERE provider = 'google') THEN i.status \
                            ELSE 'not_connected' \
                        END \
                    ELSE i.status \
                END as status, \
                i.last_synced_at, \
                CASE \
                    WHEN i.key = 'google' AND NOT EXISTS (SELECT 1 FROM credentials WHERE provider = 'google') THEN NULL \
                    ELSE i.detail \
                END as detail \
             FROM integrations i \
             ORDER BY i.label",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(IntegrationSummary {
                key: row.get(0)?,
                label: row.get(1)?,
                status: row.get(2)?,
                last_synced_at: row.get(3)?,
                detail: row.get(4)?,
            })
        })?;

        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn update_status(
        &self,
        key: &str,
        status: &str,
        detail: Option<&str>,
        last_synced_at: Option<&str>,
    ) -> Result<()> {
        let connection = self.connection.lock().expect("db lock poisoned");
        connection.execute(
            "UPDATE integrations SET status = ?2, detail = ?3, last_synced_at = COALESCE(?4, last_synced_at), updated_at = CURRENT_TIMESTAMP WHERE key = ?1",
            params![key, status, detail, last_synced_at],
        )?;
        Ok(())
    }
}
