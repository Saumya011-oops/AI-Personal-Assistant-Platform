use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

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

    /// Searches for similar points in Qdrant based on a query vector (dense retrieval).
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

    /// Searches for similar points with an optional Qdrant filter payload.
    /// This powers Faceted Retrieval (source_kind, tags, date range filtering).
    ///
    /// The `filter` argument is a Qdrant filter JSON object, e.g.:
    /// ```json
    /// { "must": [{ "key": "source", "match": { "value": "notion" } }] }
    /// ```
    pub async fn search_with_filter(
        &self,
        vector: Vec<f32>,
        limit: usize,
        filter: Option<Value>,
    ) -> Result<Vec<QdrantSearchResult>> {
        let url = format!(
            "{}/collections/{}/points/search",
            self.base_url.trim_end_matches('/'),
            self.collection_name
        );

        let mut payload = json!({
            "vector": vector,
            "limit": limit,
            "with_payload": true
        });

        if let Some(f) = filter {
            payload["filter"] = f;
        }

        let response = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Failed to search Qdrant collection '{}' with filter: {}",
                self.collection_name,
                error_text
            ));
        }

        let search_response: QdrantSearchResponse = response.json().await?;
        Ok(search_response.result)
    }

    /// Scrolls through points matching a filter without a query vector.
    /// Used for faceted browsing / listing documents by metadata attribute.
    pub async fn scroll_points(
        &self,
        filter: Option<Value>,
        limit: usize,
    ) -> Result<Vec<QdrantSearchResult>> {
        let url = format!(
            "{}/collections/{}/points/scroll",
            self.base_url.trim_end_matches('/'),
            self.collection_name
        );

        let mut payload = json!({
            "limit": limit,
            "with_payload": true,
            "with_vector": false
        });

        if let Some(f) = filter {
            payload["filter"] = f;
        }

        let response = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Failed to scroll Qdrant collection '{}': {}",
                self.collection_name,
                error_text
            ));
        }

        // Scroll response has a different structure: { result: { points: [...] } }
        let raw: Value = response.json().await?;
        let points = raw["result"]["points"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|p| {
                let id = p["id"].as_str().map(|s| s.to_string())?;
                let payload = p["payload"].clone();
                Some(QdrantSearchResult { id, score: 0.0, payload })
            })
            .collect();

        Ok(points)
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
