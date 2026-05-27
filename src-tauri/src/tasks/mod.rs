use std::time::Duration;
use chrono::{DateTime, Utc};
use tauri::async_runtime::spawn;

use crate::services::AppState;
use crate::integrations::notion::NotionIntegration;
use crate::integrations::obsidian::ObsidianIntegration;

/// Starts a background Toko task that periodically syncs active integrations
pub fn start_sync_scheduler(state: AppState) {
    spawn(async move {
        tracing::info!("Starting background sync scheduler...");

        // Wait 10 seconds after boot before the first sync check to let Tauri settle
        tokio::time::sleep(Duration::from_secs(10)).await;

        loop {
            let integrations = match state.database.integration_repository().list_integrations() {
                Ok(list) => list,
                Err(err) => {
                    tracing::error!("Sync scheduler failed to list integrations: {}", err);
                    tokio::time::sleep(Duration::from_secs(30)).await;
                    continue;
                }
            };

            for integration in integrations {
                // Only sync if the integration status is 'connected' or 'syncing' or 'error' (i.e. configured)
                if integration.status == "not_connected" {
                    continue;
                }

                let key = integration.key.as_str();
                let last_synced = integration.last_synced_at.as_deref().and_then(|ts| {
                    DateTime::parse_from_rfc3339(ts).ok().map(|dt| dt.with_timezone(&Utc))
                });

                let now = Utc::now();
                let should_sync = match last_synced {
                    None => true, // Never synced before
                    Some(last) => {
                        let elapsed = now.signed_duration_since(last);
                        match key {
                            "obsidian" => elapsed.num_minutes() >= 2, // Sync Obsidian every 2 minutes (near real-time)
                            "notion" => elapsed.num_hours() >= 1,     // Sync Notion every 1 hour (periodic)
                            _ => false,
                        }
                    }
                };

                if should_sync {
                    tracing::info!("Background sync triggered for source: {}", key);
                    let state_clone = state.clone();

                    match key {
                        "obsidian" => {
                            spawn(async move {
                                if let Err(err) = run_background_obsidian_sync(&state_clone).await {
                                    tracing::error!("Background Obsidian sync failed: {}", err);
                                }
                            });
                        }
                        "notion" => {
                            spawn(async move {
                                if let Err(err) = run_background_notion_sync(&state_clone).await {
                                    tracing::error!("Background Notion sync failed: {}", err);
                                }
                            });
                        }
                        _ => {}
                    }
                }
            }

            // Sleep 30 seconds before checking again
            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    });
}

async fn run_background_obsidian_sync(state: &AppState) -> anyhow::Result<()> {
    let settings = state.database.settings_repository().get_settings()?;
    let Some(vault_path) = settings.obsidian_vault_path else {
        return Ok(());
    };

    let integration = ObsidianIntegration::new(vault_path);
    state.sync_service.run_obsidian_sync(&state.database, &integration).await?;

    // Process all pending chunks/embeddings in background
    let pipeline = state.pipeline_service.clone();
    let db = state.database.clone();
    spawn(async move {
        if let Err(err) = pipeline.process_all_pending_documents(&db).await {
            tracing::error!("Background embedding pipeline failed after periodic Obsidian sync: {}", err);
        }
    });

    Ok(())
}

async fn run_background_notion_sync(state: &AppState) -> anyhow::Result<()> {
    let integration = NotionIntegration::new(state.config.clone());
    state.sync_service.run_notion_sync(&state.database, &integration).await?;

    // Process all pending chunks/embeddings in background
    let pipeline = state.pipeline_service.clone();
    let db = state.database.clone();
    spawn(async move {
        if let Err(err) = pipeline.process_all_pending_documents(&db).await {
            tracing::error!("Background embedding pipeline failed after periodic Notion sync: {}", err);
        }
    });

    Ok(())
}
