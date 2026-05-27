use std::sync::Arc;

use tauri::Manager;
use tokio::sync::Mutex;

use crate::commands;
use crate::config::AppConfig;
use crate::db::Database;
use crate::services::{AppState, CredentialService, SyncService};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let config = AppConfig::load()?;
            let database = Database::connect(&config.database_path)?;
            database.run_migrations()?;

            let ollama_service = crate::services::ollama::OllamaService::new(
                config.ollama_url.clone(),
                config.embedding_model.clone(),
            );
            let qdrant_service = crate::services::qdrant::QdrantService::new(
                config.qdrant_url.clone(),
                config.qdrant_collection.clone(),
            );
            let pipeline_service = Arc::new(crate::services::pipeline::PipelineService::new(
                ollama_service,
                qdrant_service,
            ));

            let pipeline_clone = pipeline_service.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(err) = pipeline_clone.initialize().await {
                    tracing::error!("Failed to initialize Qdrant vector database: {}", err);
                } else {
                    tracing::info!("Qdrant vector database initialized successfully!");
                }
            });

            let state = AppState {
                config,
                database,
                sync_service: Arc::new(SyncService::new()),
                credential_service: Arc::new(CredentialService::new(&handle)?),
                pipeline_service,
                oauth_pending_state: Arc::new(Mutex::new(None)),
                app_handle: handle,
            };

            let state_clone = state.clone();
            crate::tasks::start_sync_scheduler(state_clone);

            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::health::get_app_status,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::select_obsidian_vault,
            commands::integrations::list_integrations,
            commands::obsidian::scan_obsidian_vault,
            commands::notion::sync_notion_documents,
            commands::google_auth::connect_google,
            commands::google_auth::oauth_callback,
            commands::google_auth::get_google_auth_status,
            commands::documents::list_documents,
            commands::documents::search_documents_semantic,
            commands::documents::clear_all_documents,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run tauri application");
}
