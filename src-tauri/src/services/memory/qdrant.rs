use anyhow::Result;
use crate::services::qdrant::{QdrantService, QdrantPoint};
use serde_json::json;

pub struct MemoryQdrant {
    service: QdrantService,
}

impl MemoryQdrant {
    pub fn new(qdrant_url: &str) -> Self {
        Self {
            service: QdrantService::new(qdrant_url.to_string(), "assistant_memories".to_string()),
        }
    }

    pub async fn initialize(&self) -> Result<()> {
        match self.service.initialize_collection().await {
            Ok(_) => {
                tracing::info!("Memory Qdrant collection initialized successfully");
                Ok(())
            }
            Err(e) => {
                tracing::warn!("Failed to initialize Memory Qdrant collection: {}. Semantic memory search will be disabled.", e);
                Err(e)
            }
        }
    }

    pub async fn upsert_memory(
        &self,
        id: &str,
        vector: Vec<f32>,
        r#type: &str,
        content: &str,
        importance: i64,
    ) -> Result<()> {
        let point = QdrantPoint {
            id: id.to_string(),
            vector,
            payload: json!({
                "memory_id": id.to_string(),
                "type": r#type.to_string(),
                "content": content.to_string(),
                "importance": importance,
            }),
        };
        self.service.upsert_points(vec![point]).await
    }

    pub async fn delete_memory(&self, id: &str) -> Result<()> {
        self.service.delete_points(vec![id.to_string()]).await
    }

    pub async fn search_similar_memories(&self, vector: Vec<f32>, limit: usize) -> Result<Vec<(String, f32)>> {
        let results = self.service.search_similar_points(vector, limit, None).await?;
        let mapped = results
            .into_iter()
            .map(|r| {
                let id = r.payload.get("memory_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or(r.id);
                (id, r.score)
            })
            .collect();
        Ok(mapped)
    }

    pub async fn clear_collection(&self) -> Result<()> {
        self.service.clear_collection().await
    }
}
