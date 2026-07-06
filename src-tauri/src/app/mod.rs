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

            let credential_service = Arc::new(CredentialService::new(&handle)?);

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
                Some(database.clone()),
                Some(credential_service.clone()),
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
                ollama_service.clone(),
                qdrant_service,
                sparse_service,
                groq_service.clone(),
                query_analyzer_service,
                reranker_service,
                context_builder,
            ));
            let memory_service = Arc::new(crate::services::memory::MemoryService::new(
                database.clone(),
                ollama_service.clone(),
                groq_service,
                &config.qdrant_url,
            ));

            let pipeline_clone = pipeline_service.clone();
            let retrieval_clone = retrieval_service.clone();
            let memory_clone = memory_service.clone();
            let database_clone = database.clone();
            tauri::async_runtime::spawn(async move {
                // Pipeline initialization: Qdrant failure is non-fatal.
                // The app continues in sparse-only mode if Qdrant is unavailable.
                match pipeline_clone.initialize(&database_clone).await {
                    Ok(_) => {
                        tracing::info!("Retrieval pipeline initialized successfully");
                    }
                    Err(err) => {
                        // Check whether it is a Qdrant connectivity error or something else
                        let err_str = err.to_string();
                        if err_str.contains("Qdrant") || err_str.contains("6333") || err_str.contains("connection") {
                            tracing::warn!(
                                "[STARTUP] ⚠️  Qdrant is not reachable: {}. \
                                 Dense retrieval is DISABLED. \
                                 The assistant will use sparse-only retrieval.",
                                err
                            );
                        } else {
                            tracing::error!("Failed to initialize retrieval pipeline: {}", err);
                        }
                    }
                }

                match retrieval_clone.initialize(&database_clone).await {
                    Ok(_) => {
                        tracing::info!("Retrieval orchestrator initialized successfully");
                    }
                    Err(err) => {
                        tracing::error!("Failed to initialize retrieval orchestrator: {}", err);
                    }
                }

                // Initialize memories collection
                let _ = memory_clone.initialize().await;

                // Run Qdrant health check to produce a clear startup diagnostic
                retrieval_clone.check_dense_retrieval_health().await;
            });


            let state = AppState {
                config,
                database,
                sync_service: Arc::new(SyncService::new()),
                credential_service,
                pipeline_service,
                retrieval_service,
                memory_service,
                intent_router: crate::services::intent_router::IntentRouter::new(),
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
            commands::integrations::save_credential,
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
            commands::documents::get_rag_performance_report,
            commands::settings::logout_and_reset,
            commands::memory::create_chat,
            commands::memory::list_chats,
            commands::memory::delete_chat,
            commands::memory::rename_chat,
            commands::memory::search_chats,
            commands::memory::load_chat_messages,
            commands::memory::get_conversation_summary,
            commands::memory::list_memories,
            commands::memory::delete_memory,
            commands::memory::update_memory,
            commands::memory::clear_all_memories,
            commands::memory::export_memories,
            commands::memory::import_memories,
            commands::memory::reset_assistant_data,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run tauri application");
}
