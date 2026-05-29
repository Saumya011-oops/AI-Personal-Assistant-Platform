use anyhow::Result;
use serde_json::json;

use crate::db::Database;
use crate::domain::NormalizedDocument;
use crate::services::chunker::{ParagraphChunker, RecursiveChunker};
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

    /// Processes a single document using the standard paragraph chunker (512 tokens).
    /// Chunks, embeds, and indexes into Qdrant. This is the default pipeline used
    /// for Dense, Sparse, Hybrid, Faceted, and Contextual retrieval strategies.
    pub async fn process_document(
        &self,
        database: &Database,
        document: &NormalizedDocument,
    ) -> Result<()> {
        tracing::info!("Starting standard embedding pipeline for document: {}", document.title);

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

        // 2. Persist chunks in SQLite (also populates FTS5 via triggers)
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
                    "chunk_level": chunk.chunk_level.clone(),
                    "parent_chunk_id": chunk.parent_chunk_id.clone(),
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
            "Standard pipeline: {} chunks indexed for '{}'",
            chunks.len(),
            document.title
        );
        Ok(())
    }

    /// Processes a document using the recursive chunker (parent 1024 + child 256 tokens).
    /// Both parent and child chunks are embedded separately and indexed in Qdrant.
    /// Dense search targets child chunks; retrieval loads parent for richer context.
    pub async fn process_document_recursive(
        &self,
        database: &Database,
        document: &NormalizedDocument,
    ) -> Result<()> {
        tracing::info!("Starting recursive embedding pipeline for document: {}", document.title);

        // 1. Chunk with hierarchy (parents + children)
        let chunks = RecursiveChunker::chunk_document_recursive(
            &document.id,
            &document.content_plaintext,
            1024,
            256,
        );

        if chunks.is_empty() {
            tracing::info!("Document '{}' is empty, skipping recursive chunking", document.title);
            return Ok(());
        }

        // 2. Persist all chunks (parents first so FK constraints are satisfied)
        database.document_repository().save_chunks_recursive(&document.id, &chunks)?;

        // 3. Extract text for embedding (all levels)
        let chunk_texts: Vec<String> = chunks.iter().map(|c| c.content.clone()).collect();

        // 4. Generate embeddings in batches of 32
        let mut embeddings = Vec::new();
        for batch in chunk_texts.chunks(32) {
            let batch_embeddings = self.ollama_service.generate_embeddings(batch).await?;
            embeddings.extend(batch_embeddings);
        }

        // 5. Build Qdrant points — include chunk_level and parent_chunk_id in payload
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
                    "chunk_level": chunk.chunk_level.clone(),
                    "parent_chunk_id": chunk.parent_chunk_id.clone(),
                }),
            })
            .collect();

        // 6. Upsert into Qdrant
        self.qdrant_service.upsert_points(qdrant_points).await?;

        // 7. Update all chunk statuses
        for chunk in &chunks {
            database
                .document_repository()
                .update_chunk_status(&chunk.id, "completed")?;
        }

        let parent_count = chunks.iter().filter(|c| c.chunk_level == "parent").count();
        let child_count = chunks.iter().filter(|c| c.chunk_level == "child").count();
        tracing::info!(
            "Recursive pipeline: {} parent + {} child chunks indexed for '{}'",
            parent_count,
            child_count,
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

    /// Re-indexes all existing documents using the recursive chunker.
    /// This is triggered manually via the "Rebuild Index" UI action.
    pub async fn rebuild_recursive_index(&self, database: &Database) -> Result<usize> {
        let documents = database.document_repository().list_documents(None, None)?;
        let mut processed_count = 0;

        tracing::info!("Rebuilding recursive index for {} documents", documents.len());

        for doc in &documents {
            match self.process_document_recursive(database, doc).await {
                Ok(_) => {
                    processed_count += 1;
                }
                Err(err) => {
                    tracing::error!(
                        "Failed to build recursive index for document '{}': {}",
                        doc.title,
                        err
                    );
                }
            }
        }

        tracing::info!("Recursive index rebuild complete: {} documents processed", processed_count);
        Ok(processed_count)
    }
}
