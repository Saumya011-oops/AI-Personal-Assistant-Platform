use anyhow::Result;
use serde_json::json;

use crate::db::Database;
use crate::domain::{ChunkSearchDocument, NormalizedDocument};
use crate::services::chunker::ParagraphChunker;
use crate::services::ollama::OllamaService;
use crate::services::qdrant::{QdrantService, QdrantPoint};
use crate::services::sparse::SparseRetrievalService;

#[derive(Clone)]
pub struct PipelineService {
    ollama_service: OllamaService,
    qdrant_service: QdrantService,
    sparse_service: SparseRetrievalService,
}

impl PipelineService {
    pub fn new(
        ollama_service: OllamaService,
        qdrant_service: QdrantService,
        sparse_service: SparseRetrievalService,
    ) -> Self {
        Self {
            ollama_service,
            qdrant_service,
            sparse_service,
        }
    }

    pub fn ollama_service(&self) -> &OllamaService {
        &self.ollama_service
    }

    pub fn qdrant_service(&self) -> &QdrantService {
        &self.qdrant_service
    }

    pub fn sparse_service(&self) -> &SparseRetrievalService {
        &self.sparse_service
    }

    /// Initializes Qdrant collection on startup.
    pub async fn initialize(&self, database: &Database) -> Result<()> {
        self.qdrant_service.initialize_collection().await?;
        self.sparse_service.initialize().await?;
        let documents = database.document_repository().list_all_chunk_search_documents()?;
        self.sparse_service.rebuild_index(&documents).await?;
        Ok(())
    }

    /// Processes a single document: chunks it, estimates tokens, saves chunks to SQLite,
    /// generates embeddings via Ollama, and indexes them in Qdrant.
    /// Processes a single document: chunks it, estimates tokens, saves chunks to SQLite,
    /// generates embeddings via Ollama, and indexes them in Qdrant.
    pub async fn process_document(
        &self,
        database: &Database,
        document: &NormalizedDocument,
    ) -> Result<()> {
        tracing::info!("Starting embedding pipeline for document: {}", document.title);

        let is_markdown = document.source_kind == "obsidian"
            || document.source_kind == "notion"
            || document.path_or_url.as_ref().map_or(false, |p| {
                let lower = p.to_lowercase();
                lower.ends_with(".md") || lower.ends_with(".markdown")
            });

        let file_name = document.path_or_url.as_ref()
            .and_then(|p| std::path::Path::new(p).file_name())
            .and_then(|f| f.to_str())
            .unwrap_or(&document.title)
            .to_string();

        let chunks = if is_markdown {
            let config = crate::services::chunker::ChunkerConfig {
                use_fallback: false,
                parent_target: 2000,
                parent_max: 2500,
                child_target: 400,
                child_max: 500,
                overlap_target: 100,
            };
            ParagraphChunker::chunk_document_with_config(
                &document.id,
                &document.content_plaintext,
                &config,
                &file_name,
            )
        } else {
            let config = crate::services::chunker::ChunkerConfig {
                use_fallback: true,
                parent_target: 2000,
                parent_max: 2500,
                child_target: 400,
                child_max: 512,
                overlap_target: 100,
            };
            ParagraphChunker::chunk_document_with_config(
                &document.id,
                &document.content_plaintext,
                &config,
                &file_name,
            )
        };

        if chunks.is_empty() {
            tracing::info!("Document '{}' is empty, skipping chunking", document.title);
            return Ok(());
        }

        // 2. Persist chunks in SQLite
        database.document_repository().save_chunks(&document.id, &chunks)?;

        // Filter only chunks that require embeddings (children and standalones)
        let embeddable_chunks: Vec<&crate::domain::DocumentChunk> = chunks
            .iter()
            .filter(|c| c.embedding_status == "pending")
            .collect();

        if embeddable_chunks.is_empty() {
            tracing::info!("Document '{}' has no embeddable chunks, skipping embedding step", document.title);
            return Ok(());
        }

        // 3. Extract text content from chunks for embedding batching (summary + content)
        let chunk_texts: Vec<String> = embeddable_chunks
            .iter()
            .map(|c| {
                if let Some(ref sum) = c.summary {
                    format!("{}\n\n{}", sum, c.content)
                } else {
                    c.content.clone()
                }
            })
            .collect();

        // 4. Send to Ollama in batches of up to 32
        let mut embeddings = Vec::new();
        for batch in chunk_texts.chunks(32) {
            let batch_embeddings = self.ollama_service.generate_embeddings(batch).await?;
            embeddings.extend(batch_embeddings);
        }

        // 5. Build Qdrant points with payload metadata
        let qdrant_points: Vec<QdrantPoint> = embeddable_chunks
            .iter()
            .zip(embeddings.into_iter())
            .map(|(chunk, vector)| {
                let chunk_metadata: serde_json::Value = chunk
                    .metadata_json
                    .as_ref()
                    .and_then(|json_str| serde_json::from_str(json_str).ok())
                    .unwrap_or(serde_json::json!({}));

                QdrantPoint {
                    id: chunk.id.clone(),
                    vector,
                    payload: json!({
                        "chunk_id": chunk.id.clone(),
                        "document_id": chunk.document_id.clone(),
                        "source": document.source_kind.clone(),
                        "author": document.metadata.get("author").and_then(|value| value.as_str()).map(|s| s.to_lowercase()),
                        "category": document.metadata.get("category").and_then(|value| value.as_str()).map(|s| s.to_lowercase()),
                        "title": document.title.clone(),
                        "content": chunk.content.clone(),
                        "ordinal": chunk.ordinal,
                        "path_or_url": document.path_or_url.clone(),
                        "created_at": document.created_at.clone(),
                        "modified_at": document.updated_at.clone(),
                        "tags": document.tags.iter().map(|t| t.to_lowercase()).collect::<Vec<_>>(),
                        "metadata": document.metadata.clone(),
                        // Upgraded Chunk Metadata
                        "parent_id": chunk.parent_id.clone(),
                        "summary": chunk.summary.clone(),
                        "classification": chunk.classification.clone(),
                        "file_name": chunk_metadata.get("fileName").or_else(|| chunk_metadata.get("file_name")).cloned(),
                        "section": chunk_metadata.get("section").cloned(),
                        "subsection": chunk_metadata.get("subsection").cloned(),
                        "chunk_type": chunk_metadata.get("chunkType").or_else(|| chunk_metadata.get("chunk_type")).cloned(),
                    }),
                }
            })
            .collect();

        // 6. Upsert points into Qdrant
        self.qdrant_service.upsert_points(qdrant_points).await?;

        // 7. Update status in SQLite to 'completed' for embeddable chunks
        for chunk in &embeddable_chunks {
            database
                .document_repository()
                .update_chunk_status(&chunk.id, "completed")?;
        }

        database
            .document_repository()
            .sync_chunk_search_index(document, &chunks)?;

        let search_documents = embeddable_chunks
            .iter()
            .map(|chunk| {
                let content = if let Some(ref sum) = chunk.summary {
                    format!("{}\n\n{}", sum, chunk.content)
                } else {
                    chunk.content.clone()
                };
                ChunkSearchDocument {
                    chunk_id: chunk.id.clone(),
                    document_id: chunk.document_id.clone(),
                    ordinal: chunk.ordinal,
                    source_kind: document.source_kind.clone(),
                    title: document.title.clone(),
                    content,
                    path_or_url: document.path_or_url.clone(),
                    tags: document.tags.clone(),
                    author: document
                        .metadata
                        .get("author")
                        .and_then(|value| value.as_str())
                        .map(str::to_string),
                    category: document
                        .metadata
                        .get("category")
                        .and_then(|value| value.as_str())
                        .map(str::to_string),
                    created_at: document.created_at.clone(),
                    updated_at: document.updated_at.clone(),
                    metadata: document.metadata.clone(),
                    chunk_metadata_json: chunk.metadata_json.clone(),
                }
            })
            .collect::<Vec<_>>();
        self.sparse_service.upsert_documents(&search_documents).await?;

        tracing::info!(
            "Successfully chunked, embedded, and indexed {} chunks for '{}'",
            embeddable_chunks.len(),
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
