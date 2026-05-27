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

    pub fn save_chunks(&self, document_id: &str, chunks: &[DocumentChunk]) -> Result<()> {
        let mut connection = self.connection.lock().expect("db lock poisoned");
        let transaction = connection.transaction()?;

        // Delete existing chunks for this document first
        transaction.execute(
            "DELETE FROM chunks WHERE document_id = ?1",
            params![document_id],
        )?;

        // Insert new chunks
        for chunk in chunks {
            transaction.execute(
                "INSERT INTO chunks (id, document_id, ordinal, content, token_count, embedding_status)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    chunk.id,
                    chunk.document_id,
                    chunk.ordinal,
                    chunk.content,
                    chunk.token_count,
                    chunk.embedding_status,
                ],
            )?;
        }

        transaction.commit()?;
        Ok(())
    }

    pub fn get_chunks_by_document(&self, document_id: &str) -> Result<Vec<DocumentChunk>> {
        let connection = self.connection.lock().expect("db lock poisoned");
        let mut statement = connection.prepare(
            "SELECT id, document_id, ordinal, content, token_count, embedding_status, created_at
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
                created_at: row.get(6)?,
            })
        })?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    #[allow(dead_code)]
    pub fn get_pending_chunks(&self) -> Result<Vec<DocumentChunk>> {
        let connection = self.connection.lock().expect("db lock poisoned");
        let mut statement = connection.prepare(
            "SELECT id, document_id, ordinal, content, token_count, embedding_status, created_at
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
                created_at: row.get(6)?,
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
}

