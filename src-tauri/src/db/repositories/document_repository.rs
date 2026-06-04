use std::sync::{Arc, Mutex};

use anyhow::Result;
use rusqlite::{params, Connection};

use crate::domain::{ChunkSearchDocument, DocumentChunk, MetadataFilters, NormalizedDocument};

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

    pub fn sync_chunk_search_index(
        &self,
        document: &NormalizedDocument,
        chunks: &[DocumentChunk],
    ) -> Result<()> {
        let mut connection = self.connection.lock().expect("db lock poisoned");
        let transaction = connection.transaction()?;

        transaction.execute(
            "DELETE FROM chunk_fts WHERE document_id = ?1",
            params![document.id],
        )?;

        let author = document
            .metadata
            .get("author")
            .and_then(|value| value.as_str())
            .map(str::to_string);
        let category = document
            .metadata
            .get("category")
            .and_then(|value| value.as_str())
            .map(str::to_string);
        let tags = document.tags.join(" ");

        for chunk in chunks {
            transaction.execute(
                "INSERT INTO chunk_fts (
                    chunk_id, document_id, source, title, content, tags, author, category, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    chunk.id,
                    document.id,
                    document.source_kind,
                    document.title,
                    chunk.content,
                    tags,
                    author,
                    category,
                    document.created_at,
                    document.updated_at,
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
        connection.execute("DELETE FROM chunk_fts", [])?;
        connection.execute("UPDATE integrations SET last_synced_at = NULL, detail = NULL, status = 'connected'", [])?;
        connection.execute("DELETE FROM sync_state", [])?;
        Ok(())
    }

    pub fn get_chunk_search_documents_by_ids(
        &self,
        chunk_ids: &[String],
    ) -> Result<Vec<ChunkSearchDocument>> {
        if chunk_ids.is_empty() {
            return Ok(Vec::new());
        }

        let connection = self.connection.lock().expect("db lock poisoned");
        let placeholders = (0..chunk_ids.len())
            .map(|_| "?".to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT
                c.id,
                c.document_id,
                c.ordinal,
                d.source_kind,
                d.title,
                c.content,
                d.path_or_url,
                d.tags_json,
                d.metadata_json,
                d.created_at,
                d.updated_at
             FROM chunks c
             JOIN documents d ON d.id = c.document_id
             WHERE c.id IN ({placeholders})"
        );
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map(rusqlite::params_from_iter(chunk_ids.iter()), |row| {
            let tags_json: String = row.get(7)?;
            let metadata_json: String = row.get(8)?;
            let metadata: serde_json::Value =
                serde_json::from_str(&metadata_json).unwrap_or(serde_json::json!({}));
            Ok(ChunkSearchDocument {
                chunk_id: row.get(0)?,
                document_id: row.get(1)?,
                ordinal: row.get(2)?,
                source_kind: row.get(3)?,
                title: row.get(4)?,
                content: row.get(5)?,
                path_or_url: row.get(6)?,
                tags: serde_json::from_str(&tags_json).unwrap_or_default(),
                author: metadata
                    .get("author")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                category: metadata
                    .get("category")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
                metadata,
            })
        })?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn list_all_chunk_search_documents(&self) -> Result<Vec<ChunkSearchDocument>> {
        let connection = self.connection.lock().expect("db lock poisoned");
        let mut statement = connection.prepare(
            "SELECT
                c.id,
                c.document_id,
                c.ordinal,
                d.source_kind,
                d.title,
                c.content,
                d.path_or_url,
                d.tags_json,
                d.metadata_json,
                d.created_at,
                d.updated_at
             FROM chunks c
             JOIN documents d ON d.id = c.document_id
             ORDER BY d.id, c.ordinal ASC",
        )?;
        let rows = statement.query_map([], |row| {
            let tags_json: String = row.get(7)?;
            let metadata_json: String = row.get(8)?;
            let metadata: serde_json::Value =
                serde_json::from_str(&metadata_json).unwrap_or(serde_json::json!({}));
            Ok(ChunkSearchDocument {
                chunk_id: row.get(0)?,
                document_id: row.get(1)?,
                ordinal: row.get(2)?,
                source_kind: row.get(3)?,
                title: row.get(4)?,
                content: row.get(5)?,
                path_or_url: row.get(6)?,
                tags: serde_json::from_str(&tags_json).unwrap_or_default(),
                author: metadata
                    .get("author")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                category: metadata
                    .get("category")
                    .and_then(|value| value.as_str())
                    .map(str::to_string),
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
                metadata,
            })
        })?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn search_chunks_bm25(
        &self,
        query: &str,
        limit: usize,
        filters: Option<&MetadataFilters>,
    ) -> Result<Vec<(String, f32)>> {
        let connection = self.connection.lock().expect("db lock poisoned");
        let safe_query = query
            .split_whitespace()
            .map(|token| token.replace('"', ""))
            .filter(|token| !token.is_empty())
            .collect::<Vec<_>>()
            .join(" ");

        if safe_query.is_empty() {
            return Ok(Vec::new());
        }

        let mut sql = String::from(
            "SELECT f.chunk_id, bm25(chunk_fts) AS score
             FROM chunk_fts f
             JOIN documents d ON d.id = f.document_id
             WHERE chunk_fts MATCH ?1",
        );
        let mut params_vec: Vec<String> = vec![safe_query];

        if let Some(filters) = filters {
            if let Some(source) = &filters.source {
                let placeholders = source
                    .iter()
                    .map(|_| "?".to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                sql.push_str(&format!(" AND LOWER(d.source_kind) IN ({placeholders})"));
                params_vec.extend(source.iter().map(|s| s.to_lowercase()));
            }
            if let Some(authors) = &filters.author {
                let clauses = authors
                    .iter()
                    .map(|_| "LOWER(json_extract(d.metadata_json, '$.author')) LIKE ?".to_string())
                    .collect::<Vec<_>>()
                    .join(" OR ");
                sql.push_str(&format!(" AND ({clauses})"));
                params_vec.extend(authors.iter().map(|s| format!("%{}%", s.to_lowercase())));
            }
            if let Some(tags) = &filters.tags {
                let clauses = tags
                    .iter()
                    .map(|_| "LOWER(d.tags_json) LIKE ?".to_string())
                    .collect::<Vec<_>>()
                    .join(" OR ");
                sql.push_str(&format!(" AND ({clauses})"));
                params_vec.extend(tags.iter().map(|s| format!("%{}%", s.to_lowercase())));
            }
            if let Some(categories) = &filters.category {
                let clauses = categories
                    .iter()
                    .map(|_| "LOWER(json_extract(d.metadata_json, '$.category')) LIKE ?".to_string())
                    .collect::<Vec<_>>()
                    .join(" OR ");
                sql.push_str(&format!(" AND ({clauses})"));
                params_vec.extend(categories.iter().map(|s| format!("%{}%", s.to_lowercase())));
            }
            if let Some(date_range) = &filters.date_range {
                if let Some(from) = &date_range.from {
                    sql.push_str(" AND COALESCE(d.updated_at, d.created_at) >= ?");
                    params_vec.push(from.clone());
                }
                if let Some(to) = &date_range.to {
                    sql.push_str(" AND COALESCE(d.updated_at, d.created_at) <= ?");
                    params_vec.push(to.clone());
                }
            }
        }

        sql.push_str(" ORDER BY score LIMIT ?");
        params_vec.push(limit.to_string());

        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map(rusqlite::params_from_iter(params_vec.iter()), |row| {
            let chunk_id: String = row.get(0)?;
            let score: f32 = row.get(1)?;
            Ok((chunk_id, score))
        })?;

        Ok(rows.filter_map(Result::ok).collect())
    }

    pub fn get_all_unique_tags(&self) -> Result<std::collections::HashSet<String>> {
        let connection = self.connection.lock().expect("db lock poisoned");
        let mut statement = connection.prepare("SELECT tags_json FROM documents")?;
        let rows = statement.query_map([], |row| {
            let tags_json: String = row.get(0)?;
            Ok(serde_json::from_str::<Vec<String>>(&tags_json).unwrap_or_default())
        })?;

        let mut tags = std::collections::HashSet::new();
        for row in rows {
            if let Ok(list) = row {
                for tag in list {
                    tags.insert(tag.to_lowercase());
                }
            }
        }
        Ok(tags)
    }

    pub fn get_all_unique_categories(&self) -> Result<std::collections::HashSet<String>> {
        let connection = self.connection.lock().expect("db lock poisoned");
        let mut statement = connection.prepare("SELECT metadata_json FROM documents")?;
        let rows = statement.query_map([], |row| {
            let metadata_json: String = row.get(0)?;
            let val: serde_json::Value = serde_json::from_str(&metadata_json).unwrap_or(serde_json::json!({}));
            let cat = val.get("category")
                .or_else(|| val.get("frontmatter").and_then(|f| f.get("category")))
                .and_then(|c| c.as_str())
                .map(|s| s.to_lowercase());
            Ok(cat)
        })?;

        let mut categories = std::collections::HashSet::new();
        for row in rows {
            if let Ok(Some(cat)) = row {
                categories.insert(cat);
            }
        }
        Ok(categories)
    }
}

