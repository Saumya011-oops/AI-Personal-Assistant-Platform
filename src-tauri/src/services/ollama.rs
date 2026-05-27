use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct OllamaService {
    base_url: String,
    model: String,
    client: Client,
}

#[derive(Serialize)]
struct EmbedRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Deserialize)]
struct EmbedResponse {
    embeddings: Vec<Vec<f32>>,
}

impl OllamaService {
    pub fn new(base_url: String, model: String) -> Self {
        Self {
            base_url,
            model,
            client: Client::new(),
        }
    }

    /// Generates 768-dimensional embeddings for a batch of chunk texts.
    /// Ollama expects standard POST to /api/embed.
    pub async fn generate_embeddings(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }

        let url = format!("{}/api/embed", self.base_url.trim_end_matches('/'));
        let request_payload = EmbedRequest {
            model: self.model.clone(),
            input: inputs.to_vec(),
        };

        let response = self
            .client
            .post(&url)
            .json(&request_payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Ollama embedding request failed with status {}: {}",
                status,
                error_text
            ));
        }

        let result: EmbedResponse = response.json().await?;
        if result.embeddings.len() != inputs.len() {
            return Err(anyhow!(
                "Ollama returned mismatching number of embeddings: expected {}, got {}",
                inputs.len(),
                result.embeddings.len()
            ));
        }

        Ok(result.embeddings)
    }
}
