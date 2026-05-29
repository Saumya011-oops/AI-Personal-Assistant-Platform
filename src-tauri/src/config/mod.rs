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
            .ok()
            .filter(|val| !val.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
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
        })
    }
}
