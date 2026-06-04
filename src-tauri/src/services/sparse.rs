use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::domain::{ChunkSearchDocument, MetadataFilters, SparseSearchHit};

#[derive(Clone)]
pub struct SparseRetrievalService {
    base_url: String,
    helper_script_path: PathBuf,
    node_binary: String,
    client: Client,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SparseSearchRequest<'a> {
    query: &'a str,
    limit: usize,
    filters: Option<&'a MetadataFilters>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SparseIndexRequest<'a> {
    documents: &'a [ChunkSearchDocument],
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SparseSearchResponse {
    results: Vec<SparseSearchHit>,
}

impl SparseRetrievalService {
    pub fn new(port: u16, helper_script_path: PathBuf, node_binary: String) -> Self {
        Self {
            base_url: format!("http://127.0.0.1:{port}"),
            helper_script_path,
            node_binary,
            client: Client::new(),
        }
    }

    pub async fn initialize(&self) -> Result<()> {
        if self.health_check().await.is_ok() {
            return Ok(());
        }

        self.spawn_worker()?;
        self.wait_until_ready().await
    }

    pub async fn health_check(&self) -> Result<()> {
        let response = self
            .client
            .get(format!("{}/health", self.base_url))
            .send()
            .await
            .context("sparse worker health request failed")?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow!("sparse worker returned unhealthy status {}", response.status()))
        }
    }

    pub async fn rebuild_index(&self, documents: &[ChunkSearchDocument]) -> Result<()> {
        self.client
            .post(format!("{}/rebuild", self.base_url))
            .json(&SparseIndexRequest { documents })
            .send()
            .await?
            .error_for_status()
            .context("failed to rebuild sparse index")?;
        Ok(())
    }

    pub async fn upsert_documents(&self, documents: &[ChunkSearchDocument]) -> Result<()> {
        self.client
            .post(format!("{}/upsert", self.base_url))
            .json(&SparseIndexRequest { documents })
            .send()
            .await?
            .error_for_status()
            .context("failed to upsert sparse index documents")?;
        Ok(())
    }

    pub async fn clear_index(&self) -> Result<()> {
        self.client
            .post(format!("{}/clear", self.base_url))
            .json(&json!({}))
            .send()
            .await?
            .error_for_status()
            .context("failed to clear sparse index")?;
        Ok(())
    }

    pub async fn search(
        &self,
        query: &str,
        filters: Option<&MetadataFilters>,
        limit: usize,
    ) -> Result<Vec<SparseSearchHit>> {
        let response = self
            .client
            .post(format!("{}/search", self.base_url))
            .json(&SparseSearchRequest {
                query,
                limit,
                filters,
            })
            .send()
            .await?
            .error_for_status()
            .context("sparse search request failed")?;

        let body: SparseSearchResponse = response.json().await?;
        Ok(body.results)
    }

    fn spawn_worker(&self) -> Result<()> {
        let port = self
            .base_url
            .rsplit(':')
            .next()
            .ok_or_else(|| anyhow!("invalid sparse worker base url"))?;
        Command::new(&self.node_binary)
            .arg(&self.helper_script_path)
            .arg(port)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null())
            .spawn()
            .with_context(|| {
                format!(
                    "failed to spawn sparse worker using '{}' and script '{}'",
                    self.node_binary,
                    self.helper_script_path.display()
                )
            })?;
        Ok(())
    }

    async fn wait_until_ready(&self) -> Result<()> {
        for _ in 0..40 {
            if self.health_check().await.is_ok() {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(250));
        }
        Err(anyhow!("sparse worker did not become ready in time"))
    }
}
