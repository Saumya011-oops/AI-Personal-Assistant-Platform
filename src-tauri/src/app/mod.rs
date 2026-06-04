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
            let sparse_service = crate::services::sparse::SparseRetrievalService::new(
                config.sparse_helper_port,
                config.sparse_helper_script_path(),
                config.node_binary.clone(),
            );
            let groq_service = crate::services::groq::GroqService::new(
                config.groq_api_key.clone(),
                config.groq_base_url.clone(),
                config.groq_model_primary.clone(),
                config.groq_model_fallback.clone(),
            );
            let query_analyzer_service =
                crate::services::query_analyzer::QueryAnalyzerService::new(groq_service.clone());
            let reranker_service = crate::services::reranker::RerankerService::new(
                config.reranker_helper_port,
                config.reranker_worker_script_path(),
                config.reranker_python_path(),
                config.reranker_model.clone(),
                config.reranker_model_cache_dir.clone(),
            );
            let context_builder = crate::services::context_builder::ContextBuilder::new();
            let pipeline_service = Arc::new(crate::services::pipeline::PipelineService::new(
                ollama_service.clone(),
                qdrant_service.clone(),
                sparse_service.clone(),
            ));
            let retrieval_service = Arc::new(crate::services::retrieval::RetrievalService::new(
                ollama_service,
                qdrant_service,
                sparse_service,
                groq_service,
                query_analyzer_service,
                reranker_service,
                context_builder,
            ));

            let pipeline_clone = pipeline_service.clone();
            let retrieval_clone = retrieval_service.clone();
            let database_clone = database.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(err) = pipeline_clone.initialize(&database_clone).await {
                    tracing::error!("Failed to initialize retrieval pipeline: {}", err);
                } else {
                    tracing::info!("Retrieval pipeline initialized successfully");
                }

                if let Err(err) = retrieval_clone.initialize(&database_clone).await {
                    tracing::error!("Failed to initialize retrieval orchestrator: {}", err);
                } else {
                    tracing::info!("Retrieval orchestrator initialized successfully");
                }
            });

            let state = AppState {
                config,
                database,
                sync_service: Arc::new(SyncService::new()),
                credential_service: Arc::new(CredentialService::new(&handle)?),
                pipeline_service,
                retrieval_service,
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
            commands::documents::retrieve_documents,
            commands::documents::ask_assistant,
            commands::documents::clear_all_documents,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run tauri application");
}
