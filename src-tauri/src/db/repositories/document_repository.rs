use std::sync::{Arc, Mutex};

use anyhow::Result;
use rusqlite::{params, Connection};

use crate::domain::{DocumentChunk, NormalizedDocument};

#[derive(Clone)]
pub struct DocumentRepository {
    connection: Arc<Mutex<Connection>>,
}

impl DocumentRepository {
    pub fn new(connection: Arc<Mutex<Connection>>) -> Self {
        Self { connection }
    }

    pub fn upsert_documents(&self, documents: &[NormalizedDocument]) -> Result<i64> {
        let mut count = 0_i64;
        let connection = self.connection.lock().expect("db lock poisoned");
        for document in documents {
            let tags_json = serde_json::to_string(&document.tags)?;
            let metadata_json = serde_json::to_string(&document.metadata)?;
            connection.execute(
                "INSERT INTO documents (
                  id, source_kind, source_external_id, title, content_markdown, content_plaintext,
                  path_or_url, tags_json, checksum, metadata_json, created_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
                ON CONFLICT(source_kind, source_external_id) DO UPDATE SET
                  title = excluded.title,
                  content_markdown = excluded.content_markdown,
                  content_plaintext = excluded.content_plaintext,
                  path_or_url = excluded.path_or_url,
                  tags_json = excluded.tags_json,
                  checksum = excluded.checksum,
                  metadata_json = excluded.metadata_json,
                  created_at = excluded.created_at,
                  updated_at = excluded.updated_at,
                  ingested_at = CURRENT_TIMESTAMP",
                params![
                    document.id,
                    document.source_kind,
                    document.source_external_id,
                    document.title,
                    document.content_markdown,
                    document.content_plaintext,
                    document.path_or_url,
                    tags_json,
                    document.checksum,
                    metadata_json,
                    document.created_at,
                    document.updated_at,
                ],
            )?;
            count += 1;
        }
        Ok(count)
    }

    pub fn list_documents(
        &self,
        source_kind: Option<String>,
        query: Option<String>,
    ) -> Result<Vec<NormalizedDocument>> {
        let connection = self.connection.lock().expect("db lock poisoned");

        let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<NormalizedDocument> {
            let tags_json: String = row.get(7)?;
            let metadata_json: String = row.get(9)?;
            Ok(NormalizedDocument {
                id: row.get(0)?,
                source_kind: row.get(1)?,
                source_external_id: row.get(2)?,
                title: row.get(3)?,
                content_markdown: row.get(4)?,
                content_plaintext: row.get(5)?,
                path_or_url: row.get(6)?,
                tags: serde_json::from_str(&tags_json).unwrap_or_default(),
                checksum: row.get(8)?,
                metadata: serde_json::from_str(&metadata_json).unwrap_or(serde_json::json!({})),
                created_at: row.get(10)?,
                updated_at: row.get(11)?,
            })
        };

        let select = "SELECT id, source_kind, source_external_id, title, content_markdown, content_plaintext, path_or_url, tags_json, checksum, metadata_json, created_at, updated_at FROM documents";

        let rows = match (source_kind.as_deref(), query.as_deref()) {
            (None, None) => {
                let sql = format!("{select} ORDER BY COALESCE(updated_at, created_at) DESC");
                let mut stmt = connection.prepare(&sql)?;
                let result = stmt.query_map([], map_row)?
                    .filter_map(Result::ok)
                    .collect::<Vec<_>>();
                result
            }
            (Some(sk), None) => {
                let sql = format!("{select} WHERE source_kind = ?1 ORDER BY COALESCE(updated_at, created_at) DESC");
                let mut stmt = connection.prepare(&sql)?;
                let result = stmt.query_map(params![sk], map_row)?
                    .filter_map(Result::ok)
                    .collect::<Vec<_>>();
                result
            }
            (None, Some(q)) => {
                let like = format!("%{q}%");
                let sql = format!("{select} WHERE (title LIKE ?1 OR content_plaintext LIKE ?1) ORDER BY COALESCE(updated_at, created_at) DESC");
                let mut stmt = connection.prepare(&sql)?;
                let result = stmt.query_map(params![like], map_row)?
                    .filter_map(Result::ok)
                    .collect::<Vec<_>>();
                result
            }
            (Some(sk), Some(q)) => {
                let like = format!("%{q}%");
                let sql = format!("{select} WHERE source_kind = ?1 AND (title LIKE ?2 OR content_plaintext LIKE ?2) ORDER BY COALESCE(updated_at, created_at) DESC");
                let mut stmt = connection.prepare(&sql)?;
                let result = stmt.query_map(params![sk, like], map_row)?
                    .filter_map(Result::ok)
                    .collect::<Vec<_>>();
                result
            }
        };

        Ok(rows)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Chunk persistence (standard)
    // ─────────────────────────────────────────────────────────────────────────

    pub fn save_chunks(&self, document_id: &str, chunks: &[DocumentChunk]) -> Result<()> {
        let mut connection = self.connection.lock().expect("db lock poisoned");
        let transaction = connection.transaction()?;

        // Delete existing chunks for this document first (cascades to FTS via trigger)
        transaction.execute(
            "DELETE FROM chunks WHERE document_id = ?1",
            params![document_id],
        )?;

        // Insert new chunks
        for chunk in chunks {
            transaction.execute(
                "INSERT INTO chunks (id, document_id, ordinal, content, token_count, embedding_status, chunk_level, parent_chunk_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    chunk.id,
                    chunk.document_id,
                    chunk.ordinal,
                    chunk.content,
                    chunk.token_count,
                    chunk.embedding_status,
                    chunk.chunk_level,
                    chunk.parent_chunk_id,
                ],
            )?;
        }

        transaction.commit()?;
        Ok(())
    }

    /// Saves a hierarchical (recursive) chunk set.
    /// Parents must be inserted before children to satisfy the FK constraint.
    /// The `chunks` Vec is expected to be ordered: parents first, then children.
    pub fn save_chunks_recursive(&self, document_id: &str, chunks: &[DocumentChunk]) -> Result<()> {
        // The chunks already arrive ordered (parents before children) from RecursiveChunker.
        // We can reuse save_chunks since the INSERT statement now includes the new columns.
        self.save_chunks(document_id, chunks)
    }

    pub fn get_chunks_by_document(&self, document_id: &str) -> Result<Vec<DocumentChunk>> {
        let connection = self.connection.lock().expect("db lock poisoned");
        let mut statement = connection.prepare(
            "SELECT id, document_id, ordinal, content, token_count, embedding_status,
                    chunk_level, parent_chunk_id, created_at
             FROM chunks WHERE document_id = ?1 ORDER BY ordinal ASC",
        )?;
        let rows = statement.query_map(params![document_id], |row| {
            Ok(DocumentChunk {
                id: row.get(0)?,
                document_id: row.get(1)?,
                ordinal: row.get(2)?,
                content: row.get(3)?,
                token_count: row.get(4)?,
                embedding_status: row.get(5)?,
                chunk_level: row.get(6)?,
                parent_chunk_id: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    #[allow(dead_code)]
    pub fn get_pending_chunks(&self) -> Result<Vec<DocumentChunk>> {
        let connection = self.connection.lock().expect("db lock poisoned");
        let mut statement = connection.prepare(
            "SELECT id, document_id, ordinal, content, token_count, embedding_status,
                    chunk_level, parent_chunk_id, created_at
             FROM chunks WHERE embedding_status = 'pending'",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(DocumentChunk {
                id: row.get(0)?,
                document_id: row.get(1)?,
                ordinal: row.get(2)?,
                content: row.get(3)?,
                token_count: row.get(4)?,
                embedding_status: row.get(5)?,
                chunk_level: row.get(6)?,
                parent_chunk_id: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn update_chunk_status(&self, chunk_id: &str, status: &str) -> Result<()> {
        let connection = self.connection.lock().expect("db lock poisoned");
        connection.execute(
            "UPDATE chunks SET embedding_status = ?1 WHERE id = ?2",
            params![status, chunk_id],
        )?;
        Ok(())
    }

    pub fn clear_all_documents(&self) -> Result<()> {
        let connection = self.connection.lock().expect("db lock poisoned");
        connection.execute("DELETE FROM documents", [])?;
        connection.execute("UPDATE integrations SET last_synced_at = NULL, detail = NULL, status = 'connected'", [])?;
        connection.execute("DELETE FROM sync_state", [])?;
        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Week 4 — Sparse / BM25 Retrieval via FTS5
    // ─────────────────────────────────────────────────────────────────────────

    /// Performs BM25-ranked full-text search over chunk contents using SQLite FTS5.
    /// Returns a list of (chunk_id, document_id, bm25_rank) tuples ordered by rank
    /// (lower rank = more relevant for FTS5's BM25 scoring).
    pub fn fts_search_chunks(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<FtsChunkHit>> {
        let connection = self.connection.lock().expect("db lock poisoned");

        // Sanitize the query for FTS5: escape special chars and handle bare terms
        let safe_query = sanitize_fts_query(query);
        if safe_query.is_empty() {
            return Ok(Vec::new());
        }

        let mut stmt = connection.prepare(
            "SELECT chunk_id, document_id, bm25(chunks_fts) AS rank, content
             FROM chunks_fts
             WHERE chunks_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )?;

        let rows = stmt.query_map(params![safe_query, limit as i64], |row| {
            Ok(FtsChunkHit {
                chunk_id: row.get(0)?,
                document_id: row.get(1)?,
                bm25_score: row.get::<_, f64>(2)?,
                content: row.get(3)?,
            })
        })?;

        Ok(rows.filter_map(Result::ok).collect())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Week 4 — Contextual Retrieval: fetch surrounding chunks (sibling window)
    // ─────────────────────────────────────────────────────────────────────────

    /// Returns `window` chunks on each side of `center_ordinal` for a given document,
    /// plus the center chunk itself. Used for the Contextual Retrieval strategy.
    pub fn get_context_chunks(
        &self,
        document_id: &str,
        center_ordinal: i64,
        window: usize,
    ) -> Result<Vec<DocumentChunk>> {
        let connection = self.connection.lock().expect("db lock poisoned");
        let half = window as i64;
        let low = (center_ordinal - half).max(0);
        let high = center_ordinal + half;

        let mut stmt = connection.prepare(
            "SELECT id, document_id, ordinal, content, token_count, embedding_status,
                    chunk_level, parent_chunk_id, created_at
             FROM chunks
             WHERE document_id = ?1
               AND ordinal BETWEEN ?2 AND ?3
               AND chunk_level != 'parent'
             ORDER BY ordinal ASC",
        )?;

        let rows = stmt.query_map(params![document_id, low, high], |row| {
            Ok(DocumentChunk {
                id: row.get(0)?,
                document_id: row.get(1)?,
                ordinal: row.get(2)?,
                content: row.get(3)?,
                token_count: row.get(4)?,
                embedding_status: row.get(5)?,
                chunk_level: row.get(6)?,
                parent_chunk_id: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?;

        Ok(rows.filter_map(Result::ok).collect())
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Week 4 — Recursive Retrieval: fetch the parent summary chunk
    // ─────────────────────────────────────────────────────────────────────────

    /// Given a child chunk id, fetches the parent summary chunk (if any).
    pub fn get_parent_chunk(&self, child_chunk_id: &str) -> Result<Option<DocumentChunk>> {
        let connection = self.connection.lock().expect("db lock poisoned");

        // First look up the parent_chunk_id for this child
        let parent_id_opt: Option<String> = {
            let mut stmt = connection.prepare(
                "SELECT parent_chunk_id FROM chunks WHERE id = ?1",
            )?;
            stmt.query_row(params![child_chunk_id], |row| row.get(0)).ok()
        };

        let Some(parent_id) = parent_id_opt else {
            return Ok(None);
        };

        let mut stmt = connection.prepare(
            "SELECT id, document_id, ordinal, content, token_count, embedding_status,
                    chunk_level, parent_chunk_id, created_at
             FROM chunks WHERE id = ?1",
        )?;

        let chunk = stmt.query_row(params![parent_id], |row| {
            Ok(DocumentChunk {
                id: row.get(0)?,
                document_id: row.get(1)?,
                ordinal: row.get(2)?,
                content: row.get(3)?,
                token_count: row.get(4)?,
                embedding_status: row.get(5)?,
                chunk_level: row.get(6)?,
                parent_chunk_id: row.get(7)?,
                created_at: row.get(8)?,
            })
        }).ok();

        Ok(chunk)
    }

    /// Looks up a chunk by its id and returns its ordinal (for context window).
    pub fn get_chunk_ordinal(&self, chunk_id: &str) -> Result<Option<i64>> {
        let connection = self.connection.lock().expect("db lock poisoned");
        let mut stmt = connection.prepare("SELECT ordinal FROM chunks WHERE id = ?1")?;
        let ordinal = stmt.query_row(params![chunk_id], |row| row.get(0)).ok();
        Ok(ordinal)
    }

    /// Looks up a document's title and metadata given its id.
    pub fn get_document_meta(&self, document_id: &str) -> Result<Option<(String, String, Option<String>, Vec<String>)>> {
        // Returns (title, source_kind, path_or_url, tags)
        let connection = self.connection.lock().expect("db lock poisoned");
        let mut stmt = connection.prepare(
            "SELECT title, source_kind, path_or_url, tags_json FROM documents WHERE id = ?1"
        )?;
        let row = stmt.query_row(params![document_id], |row| {
            let tags_json: String = row.get(3)?;
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                serde_json::from_str::<Vec<String>>(&tags_json).unwrap_or_default(),
            ))
        }).ok();
        Ok(row)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Supporting types
// ─────────────────────────────────────────────────────────────────────────────

/// A result hit from the FTS5 BM25 sparse search.
#[derive(Debug, Clone)]
pub struct FtsChunkHit {
    pub chunk_id: String,
    pub document_id: String,
    /// BM25 score from SQLite FTS5 (negative; closer to 0 = more relevant)
    pub bm25_score: f64,
    pub content: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// FTS5 query sanitizer
// ─────────────────────────────────────────────────────────────────────────────

/// Sanitizes a user query string for safe use in FTS5 MATCH clauses.
/// Wraps multi-word queries in double-quotes to perform phrase search,
/// or uses a simple term search for single words.
fn sanitize_fts_query(query: &str) -> String {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // Replace FTS5 special characters that could cause syntax errors
    let safe = trimmed
        .replace('"', " ")
        .replace('*', " ")
        .replace('^', " ")
        .replace('(', " ")
        .replace(')', " ")
        .replace(':', " ")
        .replace('-', " ");

    let words: Vec<&str> = safe.split_whitespace().filter(|s| !s.is_empty()).collect();
    if words.is_empty() {
        return String::new();
    }

    if words.len() == 1 {
        // Single word: exact term + prefix variant
        format!("{} OR {}*", words[0], words[0])
    } else {
        // Multi-word: phrase match OR individual terms
        let phrase = format!("\"{}\"", words.join(" "));
        let terms = words.join(" ");
        format!("{} OR {}", phrase, terms)
    }
}
