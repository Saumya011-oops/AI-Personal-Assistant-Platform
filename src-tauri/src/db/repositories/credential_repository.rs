use std::sync::{Arc, Mutex};

use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};

use crate::domain::CredentialRecord;

#[derive(Clone)]
pub struct CredentialRepository {
    connection: Arc<Mutex<Connection>>,
}

impl CredentialRepository {
    pub fn new(connection: Arc<Mutex<Connection>>) -> Self {
        Self { connection }
    }

    pub fn upsert(&self, record: &CredentialRecord) -> Result<()> {
        let connection = self.connection.lock().expect("db lock poisoned");
        connection.execute(
            "INSERT INTO credentials (
              provider, account_identifier, encrypted_token_blob, scopes_json, expires_at, last_refresh_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(provider) DO UPDATE SET
              account_identifier = excluded.account_identifier,
              encrypted_token_blob = excluded.encrypted_token_blob,
              scopes_json = excluded.scopes_json,
              expires_at = excluded.expires_at,
              last_refresh_at = excluded.last_refresh_at,
              updated_at = CURRENT_TIMESTAMP",
            params![
                record.provider,
                record.account_identifier,
                record.encrypted_token_blob,
                serde_json::to_string(&record.scopes)?,
                record.expires_at.map(|value| value.to_rfc3339()),
                record.last_refresh_at.map(|value| value.to_rfc3339()),
            ],
        )?;
        Ok(())
    }

    pub fn get_by_provider(&self, provider: &str) -> Result<Option<CredentialRecord>> {
        let connection = self.connection.lock().expect("db lock poisoned");
        let mut statement = connection.prepare(
            "SELECT provider, account_identifier, encrypted_token_blob, scopes_json, expires_at, last_refresh_at FROM credentials WHERE provider = ?1",
        )?;
        let mut rows = statement.query([provider])?;
        if let Some(row) = rows.next()? {
            let scopes_json: String = row.get(3)?;
            let expires_at: Option<String> = row.get(4)?;
            let last_refresh_at: Option<String> = row.get(5)?;
            return Ok(Some(CredentialRecord {
                provider: row.get(0)?,
                account_identifier: row.get(1)?,
                encrypted_token_blob: row.get(2)?,
                scopes: serde_json::from_str(&scopes_json).unwrap_or_default(),
                expires_at: expires_at
                    .and_then(|value| DateTime::parse_from_rfc3339(&value).ok())
                    .map(|value| value.with_timezone(&Utc)),
                last_refresh_at: last_refresh_at
                    .and_then(|value| DateTime::parse_from_rfc3339(&value).ok())
                    .map(|value| value.with_timezone(&Utc)),
            }));
        }

        Ok(None)
    }
}
