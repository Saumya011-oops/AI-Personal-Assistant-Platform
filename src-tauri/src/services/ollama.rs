use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMsg<'a>>,
    stream: bool,
    format: &'a str,
}

#[derive(Serialize)]
struct ChatMsg<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatResponse {
    message: ChatMsgResponse,
}

#[derive(Deserialize)]
struct ChatMsgResponse {
    content: String,
}

impl OllamaService {
    pub fn new(base_url: String, model: String) -> Self {
        Self {
            base_url,
            model,
            client: Client::new(),
        }
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    /// Generates 768-dimensional embeddings for a batch of chunk texts.
    /// Ollama expects standard POST to /api/embed.
    pub async fn generate_embeddings(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }

        let url = format!("{}/api/embed", self.base_url.trim_end_matches('/'));
        let inputs_lower: Vec<String> = inputs.iter().map(|s| s.to_lowercase()).collect();
        let request_payload = EmbedRequest {
            model: self.model.clone(),
            input: inputs_lower,
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

    /// Sends a chat completion request to Ollama's /api/chat endpoint and
    /// returns the response parsed as JSON. Uses llama3 (available locally)
    /// for structured evaluation tasks. No rate limits — runs entirely locally.
    pub async fn chat_json(&self, system_prompt: &str, user_prompt: &str) -> Result<Value> {
        let url = format!("{}/api/chat", self.base_url.trim_end_matches('/'));

        // Use llama3 for judge tasks (self.model is the embedding model nomic-embed-text)
        let judge_model = "llama3";

        let payload = ChatRequest {
            model: judge_model,
            messages: vec![
                ChatMsg { role: "system", content: system_prompt },
                ChatMsg { role: "user", content: user_prompt },
            ],
            stream: false,
            format: "json",
        };

        let response = self
            .client
            .post(&url)
            .json(&payload)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Ollama chat request failed with status {}: {}",
                status,
                body
            ));
        }

        let chat_resp: ChatResponse = response.json().await?;
        let content = chat_resp.message.content;
        serde_json::from_str(&content).or_else(|_| {
            // Extract JSON object if response has surrounding text
            let start = content.find('{').ok_or_else(|| {
                serde_json::from_str::<Value>("null").unwrap_err() // dummy — overridden below
            });
            let end = content.rfind('}').ok_or_else(|| {
                serde_json::from_str::<Value>("null").unwrap_err()
            });
            match (start, end) {
                (Ok(s), Ok(e)) => serde_json::from_str(&content[s..=e]),
                _ => serde_json::from_str("null"), // will fail with proper error
            }
        }).map_err(|e| anyhow!(
            "Ollama returned invalid json: {} — body: {}",
            e,
            &content[..content.len().min(200)]
        ))
    }
}
