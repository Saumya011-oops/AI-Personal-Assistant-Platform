use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use crate::domain::RetrievedChunk;

#[derive(Clone)]
pub struct RerankerService {
    base_url: String,
    worker_script_path: PathBuf,
    python_binary: PathBuf,
    model_name: String,
    model_cache_dir: Option<PathBuf>,
    client: Client,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RerankRequest<'a> {
    query: &'a str,
    chunks: &'a [RetrievedChunk],
    limit: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RerankResponse {
    results: Vec<RerankResult>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RerankResult {
    chunk_id: String,
    score: f32,
}

impl RerankerService {
    pub fn new(
        port: u16,
        worker_script_path: PathBuf,
        python_binary: PathBuf,
        model_name: String,
        model_cache_dir: Option<PathBuf>,
    ) -> Self {
        Self {
            base_url: format!("http://127.0.0.1:{port}"),
            worker_script_path,
            python_binary,
            model_name,
            model_cache_dir,
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

    pub async fn rerank(&self, query: &str, mut chunks: Vec<RetrievedChunk>, limit: usize) -> Result<Vec<RetrievedChunk>> {
        let response = self
            .client
            .post(format!("{}/rerank", self.base_url))
            .json(&RerankRequest {
                query,
                chunks: &chunks,
                limit,
            })
            .send()
            .await?
            .error_for_status()
            .context("reranker request failed")?;

        let body: RerankResponse = response.json().await?;
        let ranked_map = body
            .results
            .iter()
            .map(|result| (result.chunk_id.clone(), result.score))
            .collect::<HashMap<_, _>>();

        chunks.retain(|chunk| ranked_map.contains_key(&chunk.chunk_id));
        for chunk in &mut chunks {
            if let Some(score) = ranked_map.get(&chunk.chunk_id) {
                chunk.score = *score;
                chunk.reranker_score = Some(*score);
            }
        }

        chunks.sort_by(|left, right| {
            let right_score = ranked_map.get(&right.chunk_id).copied().unwrap_or_default();
            let left_score = ranked_map.get(&left.chunk_id).copied().unwrap_or_default();
            right_score
                .partial_cmp(&left_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        chunks.truncate(limit);
        Ok(chunks)
    }

    async fn health_check(&self) -> Result<()> {
        let response = self
            .client
            .get(format!("{}/health", self.base_url))
            .send()
            .await
            .context("reranker health request failed")?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow!("reranker returned unhealthy status {}", response.status()))
        }
    }

    fn spawn_worker(&self) -> Result<()> {
        let mut command = Command::new(&self.python_binary);
        command
            .arg(&self.worker_script_path)
            .arg("--port")
            .arg(
                self.base_url
                    .rsplit(':')
                    .next()
                    .ok_or_else(|| anyhow!("invalid reranker base url"))?,
            )
            .arg("--model")
            .arg(&self.model_name)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .stdin(Stdio::null());

        if let Some(cache_dir) = &self.model_cache_dir {
            command.env("HF_HOME", cache_dir);
            command.env("TRANSFORMERS_CACHE", cache_dir);
        }

        command.spawn().with_context(|| {
            format!(
                "failed to spawn reranker worker using '{}' and script '{}'",
                self.python_binary.display(),
                self.worker_script_path.display()
            )
        })?;
        Ok(())
    }

    async fn wait_until_ready(&self) -> Result<()> {
        for _ in 0..120 {
            if self.health_check().await.is_ok() {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(500));
        }
        Err(anyhow!("reranker worker did not become ready in time"))
    }
}
