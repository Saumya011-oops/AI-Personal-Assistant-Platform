use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::config::AppConfig;
use crate::domain::NormalizedDocument;

#[derive(Clone)]
pub struct NotionIntegration {
    config: AppConfig,
    client: Client,
}

impl NotionIntegration {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config,
            client: Client::new(),
        }
    }

    pub async fn fetch_documents(&self, token: &str) -> Result<Vec<NormalizedDocument>> {
        let mut all_results: Vec<Value> = Vec::new();
        let mut start_cursor: Option<String> = None;

        // Paginate through all results
        loop {
            let mut body = serde_json::json!({
                "page_size": 100,
                "sort": {
                  "direction": "descending",
                  "timestamp": "last_edited_time"
                }
            });

            if let Some(ref cursor) = start_cursor {
                body["start_cursor"] = Value::String(cursor.clone());
            }

            let response = self
                .client
                .post(format!("{}/search", self.config.notion_api_base_url))
                .header("Authorization", format!("Bearer {token}"))
                .header("Notion-Version", &self.config.notion_api_version)
                .json(&body)
                .send()
                .await?
                .error_for_status()?;

            let payload: Value = response.json().await?;
            let results = payload["results"]
                .as_array()
                .cloned()
                .unwrap_or_default();

            all_results.extend(results);

            let has_more = payload["has_more"].as_bool().unwrap_or(false);
            if !has_more {
                break;
            }
            start_cursor = payload["next_cursor"].as_str().map(String::from);
            if start_cursor.is_none() {
                break;
            }
        }

        // For pages, fetch their block content asynchronously
        let mut documents = Vec::new();
        for item in all_results {
            let object_type = item["object"].as_str().unwrap_or("");
            let page_id = item["id"].as_str().unwrap_or("").to_string();

            // Fetch block content for pages (not databases)
            let content = if object_type == "page" {
                self.fetch_page_content(&token, &page_id).await.unwrap_or_default()
            } else {
                String::new()
            };

            documents.push(self.normalize_document(item, content));
        }

        Ok(documents)
    }

    /// Fetch all text blocks from a Notion page
    async fn fetch_page_content(&self, token: &str, page_id: &str) -> Result<String> {
        let response = self
            .client
            .get(format!(
                "{}/blocks/{}/children?page_size=100",
                self.config.notion_api_base_url, page_id
            ))
            .header("Authorization", format!("Bearer {token}"))
            .header("Notion-Version", &self.config.notion_api_version)
            .send()
            .await?
            .error_for_status()?;

        let payload: Value = response.json().await?;
        let blocks = payload["results"].as_array().cloned().unwrap_or_default();

        let mut lines = Vec::new();
        for block in &blocks {
            if let Some(text) = self.extract_block_text(block) {
                if !text.is_empty() {
                    lines.push(text);
                }
            }
        }

        Ok(lines.join("\n"))
    }

    /// Extract plain text from a block object
    fn extract_block_text(&self, block: &Value) -> Option<String> {
        let block_type = block["type"].as_str()?;
        let rich_text = &block[block_type]["rich_text"];
        if let Some(parts) = rich_text.as_array() {
            let text = parts
                .iter()
                .filter_map(|part| part["plain_text"].as_str())
                .collect::<Vec<_>>()
                .join("");
            Some(text)
        } else {
            None
        }
    }

    fn normalize_document(&self, item: Value, content: String) -> NormalizedDocument {
        // Try multiple title extraction strategies
        let title = self.extract_title(&item);

        // Use fetched content if available, otherwise fall back to JSON
        let plaintext = if content.is_empty() {
            // For databases: extract property values as text
            self.extract_database_content(&item)
        } else {
            content.clone()
        };

        let content_markdown = plaintext.clone();
        let checksum = format!("{:x}", Sha256::digest(plaintext.as_bytes()));

        NormalizedDocument {
            id: Uuid::new_v4().to_string(),
            source_kind: "notion".to_string(),
            source_external_id: item["id"].as_str().unwrap_or_default().to_string(),
            title,
            content_markdown,
            content_plaintext: plaintext,
            path_or_url: item["url"].as_str().map(ToString::to_string),
            tags: Vec::new(),
            created_at: item["created_time"].as_str().map(ToString::to_string),
            updated_at: item["last_edited_time"].as_str().map(ToString::to_string),
            checksum,
            metadata: item,
        }
    }

    fn extract_title(&self, item: &Value) -> String {
        // Strategy 1: look for title-type property (pages)
        if let Some(properties) = item["properties"].as_object() {
            // Check "Name", "Title", or any title-type property
            for key in &["Name", "Title", "title", "name"] {
                if let Some(prop) = properties.get(*key) {
                    let t = self.extract_rich_text_from_prop(prop);
                    if !t.is_empty() {
                        return t;
                    }
                }
            }
            // Scan all properties for title type
            for prop in properties.values() {
                if prop["type"].as_str() == Some("title") {
                    let t = self.extract_rich_text_from_prop(prop);
                    if !t.is_empty() {
                        return t;
                    }
                }
            }
        }

        // Strategy 2: top-level title array (for databases themselves)
        if let Some(parts) = item["title"].as_array() {
            let t = parts
                .iter()
                .filter_map(|p| p["plain_text"].as_str())
                .collect::<Vec<_>>()
                .join("");
            if !t.is_empty() {
                return t;
            }
        }

        "Untitled Notion Page".to_string()
    }

    fn extract_rich_text_from_prop(&self, prop: &Value) -> String {
        // title type has prop["title"] array
        let arr = if let Some(a) = prop["title"].as_array() {
            a
        } else if let Some(a) = prop["rich_text"].as_array() {
            a
        } else {
            return String::new();
        };
        arr.iter()
            .filter_map(|p| p["plain_text"].as_str())
            .collect::<Vec<_>>()
            .join("")
    }

    fn extract_database_content(&self, item: &Value) -> String {
        let mut parts = Vec::new();
        if let Some(properties) = item["properties"].as_object() {
            for (key, prop) in properties {
                let prop_type = prop["type"].as_str().unwrap_or("");
                let value = match prop_type {
                    "title" | "rich_text" => self.extract_rich_text_from_prop(prop),
                    "select" => prop["select"]["name"]
                        .as_str()
                        .unwrap_or("")
                        .to_string(),
                    "multi_select" => prop["multi_select"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|v| v["name"].as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        })
                        .unwrap_or_default(),
                    "number" => prop["number"]
                        .as_f64()
                        .map(|n| n.to_string())
                        .unwrap_or_default(),
                    "checkbox" => prop["checkbox"]
                        .as_bool()
                        .map(|b| b.to_string())
                        .unwrap_or_default(),
                    "date" => prop["date"]["start"]
                        .as_str()
                        .unwrap_or("")
                        .to_string(),
                    "url" => prop["url"].as_str().unwrap_or("").to_string(),
                    "email" => prop["email"].as_str().unwrap_or("").to_string(),
                    _ => String::new(),
                };
                if !value.is_empty() {
                    parts.push(format!("{}: {}", key, value));
                }
            }
        }
        parts.join("\n")
    }
}
