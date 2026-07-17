use anyhow::Result;
use crate::services::qdrant::{QdrantService, QdrantPoint};
use serde_json::json;
use uuid::Uuid;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

pub struct MemoryQdrant {
    service: QdrantService,
}

/// Convert a memory's string ID to a Qdrant-compatible UUID.
/// For IDs that are already valid UUIDs, parses them directly.
/// For arbitrary string IDs (e.g., eval fixtures like "eval-mem-ltr-001"),
/// derives a deterministic fake-UUID from two u64 hashes so the same
/// string always maps to the same Qdrant point ID.
fn to_qdrant_uuid(id: &str) -> String {
    if let Ok(uuid) = Uuid::parse_str(id) {
        return uuid.to_string();
    }
    // Deterministic: hash the string with two different seeds to fill 128 bits
    let h1 = {
        let mut h = DefaultHasher::new();
        id.hash(&mut h);
        h.finish()
    };
    let h2 = {
        let mut h = DefaultHasher::new();
        format!("_salt_{}", id).hash(&mut h);
        h.finish()
    };
    // Format as a valid UUID (version bits set to 4, variant to 0b10)
    let b: [u8; 16] = {
        let mut arr = [0u8; 16];
        arr[..8].copy_from_slice(&h1.to_le_bytes());
        arr[8..].copy_from_slice(&h2.to_le_bytes());
        arr[6] = (arr[6] & 0x0F) | 0x40; // version 4
        arr[8] = (arr[8] & 0x3F) | 0x80; // variant 10xx
        arr
    };
    Uuid::from_bytes(b).to_string()
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
        // Qdrant only accepts UUID or integer point IDs.
        // Convert string IDs (including eval fixture IDs like "eval-mem-ltr-001")
        // to deterministic UUIDs so the original string ID is preserved in payload.
        let qdrant_id = to_qdrant_uuid(id);

        let point = QdrantPoint {
            id: qdrant_id,
            vector,
            payload: json!({
                "memory_id": id.to_string(),   // original string ID for lookup
                "type": r#type.to_string(),
                "content": content.to_string(),
                "importance": importance,
            }),
        };
        self.service.upsert_points(vec![point]).await
    }

    pub async fn delete_memory(&self, id: &str) -> Result<()> {
        // Convert to same UUID that was used at upsert time
        let qdrant_id = to_qdrant_uuid(id);
        self.service.delete_points(vec![qdrant_id]).await
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
