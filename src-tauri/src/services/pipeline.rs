use anyhow::Result;
use serde_json::json;

use crate::db::Database;
use crate::domain::NormalizedDocument;
use crate::services::chunker::ParagraphChunker;
use crate::services::ollama::OllamaService;
use crate::services::qdrant::{QdrantService, QdrantPoint};

#[derive(Clone)]
pub struct PipelineService {
    ollama_service: OllamaService,
    qdrant_service: QdrantService,
}

impl PipelineService {
    pub fn new(ollama_service: OllamaService, qdrant_service: QdrantService) -> Self {
        Self {
            ollama_service,
            qdrant_service,
        }
    }

    pub fn ollama_service(&self) -> &OllamaService {
        &self.ollama_service
    }

    pub fn qdrant_service(&self) -> &QdrantService {
        &self.qdrant_service
    }

    /// Initializes Qdrant collection on startup.
    pub async fn initialize(&self) -> Result<()> {
        self.qdrant_service.initialize_collection().await
    }

    /// Processes a single document: chunks it, estimates tokens, saves chunks to SQLite,
    /// generates embeddings via Ollama, and indexes them in Qdrant.
    pub async fn process_document(
        &self,
        database: &Database,
        document: &NormalizedDocument,
    ) -> Result<()> {
        tracing::info!("Starting embedding pipeline for document: {}", document.title);

        // 1. Chunk document content (paragraph-based, max 512 tokens)
        let chunks = ParagraphChunker::chunk_document(
            &document.id,
            &document.content_plaintext,
            512,
        );

        if chunks.is_empty() {
            tracing::info!("Document '{}' is empty, skipping chunking", document.title);
            return Ok(());
        }

        // 2. Persist chunks in SQLite with status 'pending'
        database.document_repository().save_chunks(&document.id, &chunks)?;

        // 3. Extract text content from chunks for embedding batching
        let chunk_texts: Vec<String> = chunks.iter().map(|c| c.content.clone()).collect();

        // 4. Send to Ollama in batches of up to 32
        let mut embeddings = Vec::new();
        for batch in chunk_texts.chunks(32) {
            let batch_embeddings = self.ollama_service.generate_embeddings(batch).await?;
            embeddings.extend(batch_embeddings);
        }

        // 5. Build Qdrant points with payload metadata
        let qdrant_points: Vec<QdrantPoint> = chunks
            .iter()
            .zip(embeddings.into_iter())
            .map(|(chunk, vector)| QdrantPoint {
                id: chunk.id.clone(),
                vector,
                payload: json!({
                    "chunk_id": chunk.id.clone(),
                    "document_id": chunk.document_id.clone(),
                    "source": document.source_kind.clone(),
                    "title": document.title.clone(),
                    "content": chunk.content.clone(),
                    "path_or_url": document.path_or_url.clone(),
                    "created_at": document.created_at.clone(),
                    "modified_at": document.updated_at.clone(),
                    "tags": document.tags.clone(),
                }),
            })
            .collect();

        // 6. Upsert points into Qdrant
        self.qdrant_service.upsert_points(qdrant_points).await?;

        // 7. Update status in SQLite to 'completed'
        for chunk in &chunks {
            database
                .document_repository()
                .update_chunk_status(&chunk.id, "completed")?;
        }

        tracing::info!(
            "Successfully chunked, embedded, and indexed {} chunks for '{}'",
            chunks.len(),
            document.title
        );
        Ok(())
    }

    /// Iterates over all documents in SQLite and processes any that do not have completed chunks.
    pub async fn process_all_pending_documents(&self, database: &Database) -> Result<usize> {
        let documents = database.document_repository().list_documents(None, None)?;
        let mut processed_count = 0;

        for doc in documents {
            let existing_chunks = database.document_repository().get_chunks_by_document(&doc.id)?;
            
            // If document has no chunks, or contains any chunks that are still 'pending'
            let needs_processing = existing_chunks.is_empty() 
                || existing_chunks.iter().any(|c| c.embedding_status == "pending");

            if needs_processing {
                match self.process_document(database, &doc).await {
                    Ok(_) => {
                        processed_count += 1;
                    }
                    Err(err) => {
                        tracing::error!(
                            "Failed to process embeddings for document '{}': {}",
                            doc.title,
                            err
                        );
                        // Continue to other documents despite single failure
                    }
                }
            }
        }

        Ok(processed_count)
    }
}
