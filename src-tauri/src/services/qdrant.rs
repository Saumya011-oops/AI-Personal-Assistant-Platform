use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Clone)]
pub struct QdrantService {
    base_url: String,
    collection_name: String,
    client: Client,
}

#[derive(Serialize)]
pub struct QdrantPoint {
    pub id: String,
    pub vector: Vec<f32>,
    pub payload: serde_json::Value,
}

impl QdrantService {
    pub fn new(base_url: String, collection_name: String) -> Self {
        Self {
            base_url,
            collection_name,
            client: Client::new(),
        }
    }

    /// Initializes the Qdrant collection if it does not already exist.
    pub async fn initialize_collection(&self) -> Result<()> {
        let url = format!(
            "{}/collections/{}",
            self.base_url.trim_end_matches('/'),
            self.collection_name
        );

        // Check if collection exists
        let check_response = self.client.get(&url).send().await?;
        if check_response.status().is_success() {
            tracing::info!("Qdrant collection '{}' already exists", self.collection_name);
            return Ok(());
        }

        // If not found (404), create it
        if check_response.status().as_u16() == 404 {
            tracing::info!("Qdrant collection '{}' not found, creating...", self.collection_name);
            let create_payload = json!({
                "vectors": {
                    "size": 768,
                    "distance": "Cosine"
                }
            });

            let create_response = self
                .client
                .put(&url)
                .json(&create_payload)
                .send()
                .await?;

            if !create_response.status().is_success() {
                let error_text = create_response.text().await.unwrap_or_default();
                return Err(anyhow!(
                    "Failed to create Qdrant collection '{}': {}",
                    self.collection_name,
                    error_text
                ));
            }

            tracing::info!("Successfully created Qdrant collection '{}'", self.collection_name);
            Ok(())
        } else {
            let error_text = check_response.text().await.unwrap_or_default();
            Err(anyhow!(
                "Failed to check Qdrant collection availability: {}",
                error_text
            ))
        }
    }

    /// Upserts a batch of vectors (points) into Qdrant.
    pub async fn upsert_points(&self, points: Vec<QdrantPoint>) -> Result<()> {
        if points.is_empty() {
            return Ok(());
        }

        let url = format!(
            "{}/collections/{}/points",
            self.base_url.trim_end_matches('/'),
            self.collection_name
        );

        let payload = json!({
            "points": points
        });

        let response = self
            .client
            .put(&url)
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Failed to upsert points to Qdrant collection '{}': {}",
                self.collection_name,
                error_text
            ));
        }

        Ok(())
    }

    /// Searches for similar points in Qdrant based on a query vector.
    pub async fn search_similar_points(&self, vector: Vec<f32>, limit: usize) -> Result<Vec<QdrantSearchResult>> {
        let url = format!(
            "{}/collections/{}/points/search",
            self.base_url.trim_end_matches('/'),
            self.collection_name
        );

        let payload = json!({
            "vector": vector,
            "limit": limit,
            "with_payload": true
        });

        let response = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Failed to search Qdrant collection '{}': {}",
                self.collection_name,
                error_text
            ));
        }

        let search_response: QdrantSearchResponse = response.json().await?;
        Ok(search_response.result)
    }

    /// Deletes and recreates the Qdrant collection to clear all vectors.
    pub async fn clear_collection(&self) -> Result<()> {
        let url = format!(
            "{}/collections/{}",
            self.base_url.trim_end_matches('/'),
            self.collection_name
        );

        let delete_response = self.client.delete(&url).send().await?;
        if delete_response.status().is_success() || delete_response.status().as_u16() == 404 {
            tracing::info!("Successfully deleted Qdrant collection '{}'", self.collection_name);
            // Re-initialize it
            self.initialize_collection().await?;
            Ok(())
        } else {
            let error_text = delete_response.text().await.unwrap_or_default();
            Err(anyhow!(
                "Failed to delete Qdrant collection '{}': {}",
                self.collection_name,
                error_text
            ))
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QdrantSearchResult {
    pub id: String,
    pub score: f32,
    pub payload: serde_json::Value,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct QdrantSearchResponse {
    result: Vec<QdrantSearchResult>,
}
