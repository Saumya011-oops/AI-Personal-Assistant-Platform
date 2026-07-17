use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use std::sync::Arc;
use crate::db::Database;
use super::CredentialService;

/// Maximum number of retries when Groq returns 429 Too Many Requests.
const MAX_RATE_LIMIT_RETRIES: u32 = 3;

/// Default backoff (seconds) if we cannot parse retry-after from error body.
const DEFAULT_BACKOFF_SECS: u64 = 22;

#[derive(Clone)]
pub struct GroqService {
    api_key: Option<String>,
    database: Option<Database>,
    credential_service: Option<Arc<CredentialService>>,
    base_url: String,
    primary_model: String,
    fallback_model: String,
    client: Client,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatMessageResponse,
}

#[derive(Deserialize)]
struct ChatMessageResponse {
    content: String,
}

impl GroqService {
    pub fn new(
        api_key: Option<String>,
        database: Option<Database>,
        credential_service: Option<Arc<CredentialService>>,
        base_url: String,
        primary_model: String,
        fallback_model: String,
    ) -> Self {
        Self {
            api_key,
            database,
            credential_service,
            base_url,
            primary_model,
            fallback_model,
            client: Client::new(),
        }
    }

    fn get_api_key(&self) -> Option<String> {
        if let (Some(db), Some(cred_svc)) = (&self.database, &self.credential_service) {
            if let Ok(Some(record)) = db.credential_repository().get_by_provider("groq") {
                if let Ok(decrypted) = cred_svc.decrypt(&record.encrypted_token_blob) {
                    return Some(decrypted);
                }
            }
        }
        self.api_key.clone()
    }

    pub fn is_configured(&self) -> bool {
        self.get_api_key()
            .map(|key| !key.trim().is_empty())
            .unwrap_or(false)
    }

    pub async fn chat_json(&self, system_prompt: &str, user_prompt: &str) -> Result<Value> {
        let response = self
            .chat_completion(system_prompt, user_prompt, true)
            .await
            .context("groq json completion failed")?;
        serde_json::from_str(&response)
            .or_else(|_| self.extract_json_object(&response))
            .context("groq returned invalid json")
    }

    pub async fn chat_text(&self, system_prompt: &str, user_prompt: &str) -> Result<String> {
        self.chat_completion(system_prompt, user_prompt, false).await
    }

    async fn chat_completion(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        json_mode: bool,
    ) -> Result<String> {
        let Some(api_key) = self.get_api_key() else {
            return Err(anyhow!("GROQ_API_KEY is not configured"));
        };

        let models = [&self.primary_model, &self.fallback_model];
        let mut last_error = None;

        for model in models {
            let payload = json!({
                "model": model,
                "temperature": 0,
                "messages": [
                    ChatMessage {
                        role: "system",
                        content: system_prompt,
                    },
                    ChatMessage {
                        role: "user",
                        content: user_prompt,
                    }
                ],
                "response_format": if json_mode {
                    json!({ "type": "json_object" })
                } else {
                    json!(null)
                }
            });

            // Retry loop for 429 rate-limit responses
            let mut attempt = 0u32;
            loop {
                let response = self
                    .client
                    .post(format!(
                        "{}/chat/completions",
                        self.base_url.trim_end_matches('/')
                    ))
                    .bearer_auth(&api_key)
                    .json(&payload)
                    .send()
                    .await;

                match response {
                    Ok(resp) if resp.status().is_success() => {
                        let body: ChatCompletionResponse = resp.json().await?;
                        if let Some(choice) = body.choices.into_iter().next() {
                            return Ok(choice.message.content);
                        }
                        last_error = Some(anyhow!("groq returned no choices"));
                        break;
                    }
                    Ok(resp) if resp.status().as_u16() == 429
                        && attempt < MAX_RATE_LIMIT_RETRIES =>
                    {
                        let body = resp.text().await.unwrap_or_default();
                        // Parse "Please try again in Xs" from Groq error message
                        let wait_secs = parse_retry_after_secs(&body)
                            .unwrap_or(DEFAULT_BACKOFF_SECS);
                        tracing::warn!(
                            "[GROQ_RATE_LIMIT] 429 model={} attempt={}/{} — waiting {}s. body={}",
                            model,
                            attempt + 1,
                            MAX_RATE_LIMIT_RETRIES,
                            wait_secs,
                            &body[..body.len().min(160)],
                        );
                        tokio::time::sleep(tokio::time::Duration::from_secs(wait_secs)).await;
                        attempt += 1;
                    }
                    Ok(resp) => {
                        let status = resp.status();
                        let body = resp.text().await.unwrap_or_default();
                        last_error =
                            Some(anyhow!("groq request failed with status {status}: {body}"));
                        break;
                    }
                    Err(e) if attempt < MAX_RATE_LIMIT_RETRIES => {
                        // Network-level error (connection refused, timeout, DNS).
                        // Retry with a short backoff — these are usually transient.
                        tracing::warn!(
                            "[GROQ_NET_ERROR] model={} attempt={}/{} — retrying in 3s. err={:?}",
                            model, attempt + 1, MAX_RATE_LIMIT_RETRIES, e
                        );
                        tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                        attempt += 1;
                    }
                    Err(error) => {
                        last_error = Some(error.into());
                        break;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("groq completion failed")))
    }

    fn extract_json_object(&self, text: &str) -> Result<Value> {
        let start = text.find('{').ok_or_else(|| anyhow!("missing json object"))?;
        let end = text.rfind('}').ok_or_else(|| anyhow!("missing json object end"))?;
        serde_json::from_str(&text[start..=end]).context("failed to parse extracted json")
    }
}

/// Parses the retry-after duration from a Groq 429 error body.
///
/// Groq error messages contain text like:
///   "Please try again in 16.099999999s."
/// We extract the seconds, round up, and add 1s buffer.
fn parse_retry_after_secs(body: &str) -> Option<u64> {
    let marker = "try again in ";
    let start = body.find(marker)? + marker.len();
    let rest = &body[start..];
    let end = rest.find('s')?;
    let secs: f64 = rest[..end].trim().parse().ok()?;
    Some(secs.ceil() as u64 + 1)
}
