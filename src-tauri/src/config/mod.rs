use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result};

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub app_env: String,
    pub data_dir: PathBuf,
    pub database_path: PathBuf,
    pub notion_api_base_url: String,
    pub notion_api_version: String,
    pub notion_token: Option<String>,
    pub google_client_id: Option<String>,
    pub google_client_secret: Option<String>,
    pub google_redirect_uri: String,
    pub google_auth_scopes: String,
    pub app_secret_key: String,
    pub ollama_url: String,
    pub embedding_model: String,
    pub qdrant_url: String,
    pub qdrant_collection: String,
    pub groq_api_key: Option<String>,
    pub groq_base_url: String,
    pub groq_model_primary: String,
    pub groq_model_fallback: String,
    pub reranker_model: String,
    pub sparse_helper_port: u16,
    pub reranker_helper_port: u16,
    pub node_binary: String,
    pub reranker_python_binary: String,
    pub reranker_model_cache_dir: Option<PathBuf>,
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        let env_paths = [
            std::path::PathBuf::from(".env"),
            std::path::PathBuf::from("src-tauri/.env"),
            std::path::PathBuf::from("../.env"),
        ];
        for path in &env_paths {
            if let Ok(content) = std::fs::read_to_string(path) {
                for line in content.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    if let Some((key, val)) = line.split_once('=') {
                        let mut val = val.trim();
                        if (val.starts_with('"') && val.ends_with('"'))
                            || (val.starts_with('\'') && val.ends_with('\''))
                        {
                            val = &val[1..val.len() - 1];
                        }
                        std::env::set_var(key.trim(), val);
                    }
                }
                break;
            }
        }

        let data_dir = env::var("APP_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                dirs::data_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join("assistant-core")
            });

        std::fs::create_dir_all(&data_dir).context("failed to create app data directory")?;

        let database_path = data_dir.join("assistant.db");

        Ok(Self {
            app_env: env::var("VITE_APP_ENV").unwrap_or_else(|_| "development".to_string()),
            data_dir,
            database_path,
            notion_api_base_url: env::var("NOTION_API_BASE_URL")
                .unwrap_or_else(|_| "https://api.notion.com/v1".to_string()),
            notion_api_version: env::var("NOTION_API_VERSION")
                .unwrap_or_else(|_| "2022-06-28".to_string()),
            notion_token: env::var("NOTION_TOKEN").ok(),
            google_client_id: env::var("GOOGLE_CLIENT_ID").ok(),
            google_client_secret: env::var("GOOGLE_CLIENT_SECRET").ok(),
            google_redirect_uri: env::var("GOOGLE_REDIRECT_URI")
                .unwrap_or_else(|_| "http://127.0.0.1:4545/oauth/google/callback".to_string()),
            google_auth_scopes: env::var("GOOGLE_AUTH_SCOPES")
                .unwrap_or_else(|_| "openid email profile".to_string()),
            app_secret_key: env::var("APP_SECRET_KEY")
                .unwrap_or_else(|_| "development-secret-key-change-me".to_string()),
            ollama_url: env::var("OLLAMA_URL")
                .unwrap_or_else(|_| "http://localhost:11434".to_string()),
            embedding_model: env::var("EMBEDDING_MODEL")
                .unwrap_or_else(|_| "nomic-embed-text".to_string()),
            qdrant_url: env::var("VECTOR_DB_URL")
                .unwrap_or_else(|_| "http://localhost:6333".to_string()),
            qdrant_collection: env::var("QDRANT_COLLECTION")
                .unwrap_or_else(|_| "assistant_documents".to_string()),
            groq_api_key: env::var("GROQ_API_KEY").ok(),
            groq_base_url: env::var("GROQ_BASE_URL")
                .unwrap_or_else(|_| "https://api.groq.com/openai/v1".to_string()),
            groq_model_primary: env::var("GROQ_MODEL_PRIMARY")
                .unwrap_or_else(|_| "llama-3.3-70b-versatile".to_string()),
            groq_model_fallback: env::var("GROQ_MODEL_FALLBACK")
                .unwrap_or_else(|_| "llama-3.1-8b-instant".to_string()),
            reranker_model: env::var("RERANKER_MODEL")
                .unwrap_or_else(|_| "cross-encoder/ms-marco-MiniLM-L-6-v2".to_string()),
            sparse_helper_port: env::var("SPARSE_HELPER_PORT")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(8741),
            reranker_helper_port: env::var("RERANKER_HELPER_PORT")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(8742),
            node_binary: env::var("NODE_BINARY").unwrap_or_else(|_| "node".to_string()),
            reranker_python_binary: env::var("RERANKER_PYTHON_BINARY")
                .unwrap_or_else(|_| ".venv-reranker/bin/python".to_string()),
            reranker_model_cache_dir: env::var("RERANKER_MODEL_CACHE_DIR").ok().map(PathBuf::from),
        })
    }

    pub fn workspace_root(&self) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
    }

    pub fn sparse_helper_script_path(&self) -> PathBuf {
        self.workspace_root()
            .join("src-tauri")
            .join("helpers")
            .join("sparse_worker.cjs")
    }

    pub fn reranker_worker_script_path(&self) -> PathBuf {
        self.workspace_root()
            .join("src-tauri")
            .join("helpers")
            .join("reranker_worker.py")
    }

    pub fn reranker_python_path(&self) -> PathBuf {
        let configured = PathBuf::from(&self.reranker_python_binary);
        if configured.is_absolute() {
            configured
        } else {
            self.workspace_root().join(configured)
        }
    }
}
