use std::sync::{Arc, Mutex};

use anyhow::Result;
use rusqlite::{params, Connection};

use crate::domain::{AppSettings, UpdateSettingsInput};

#[derive(Clone)]
pub struct SettingsRepository {
    connection: Arc<Mutex<Connection>>,
}

impl SettingsRepository {
    pub fn new(connection: Arc<Mutex<Connection>>) -> Self {
        Self { connection }
    }

    pub fn get_settings(&self) -> Result<AppSettings> {
        let connection = self.connection.lock().expect("db lock poisoned");
        let settings = connection.query_row(
            "SELECT obsidian_vault_path, preferred_theme, command_palette_enabled, telemetry_enabled FROM settings WHERE id = 1",
            [],
            |row| {
                Ok(AppSettings {
                    obsidian_vault_path: row.get(0)?,
                    preferred_theme: row.get(1)?,
                    command_palette_enabled: row.get::<_, i64>(2)? == 1,
                    telemetry_enabled: row.get::<_, i64>(3)? == 1,
                })
            },
        )?;

        Ok(settings)
    }

    pub fn update_settings(&self, input: UpdateSettingsInput) -> Result<AppSettings> {
        let current = self.get_settings()?;
        let next = AppSettings {
            obsidian_vault_path: input.obsidian_vault_path.or(current.obsidian_vault_path),
            preferred_theme: input.preferred_theme.unwrap_or(current.preferred_theme),
            command_palette_enabled: input
                .command_palette_enabled
                .unwrap_or(current.command_palette_enabled),
            telemetry_enabled: input.telemetry_enabled.unwrap_or(current.telemetry_enabled),
        };

        let connection = self.connection.lock().expect("db lock poisoned");
        connection.execute(
            "UPDATE settings
             SET obsidian_vault_path = ?1, preferred_theme = ?2, command_palette_enabled = ?3, telemetry_enabled = ?4, updated_at = CURRENT_TIMESTAMP
             WHERE id = 1",
            params![
                next.obsidian_vault_path,
                next.preferred_theme,
                i64::from(next.command_palette_enabled),
                i64::from(next.telemetry_enabled)
            ],
        )?;

        Ok(next)
    }
}
