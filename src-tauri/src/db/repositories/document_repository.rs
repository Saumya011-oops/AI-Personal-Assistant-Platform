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
            let mut metadata = document.metadata.clone();
            crate::domain::enrich_metadata(&document.title, &document.id, &mut metadata);
            let tags_json = serde_json::to_string(&document.tags)?;
            let metadata_json = serde_json::to_string(&metadata)?;
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

    pub fn get_document_by_id(&self, id: &str) -> Result<Option<NormalizedDocument>> {
        let connection = self.connection.lock().expect("db lock poisoned");
        let select = "SELECT id, source_kind, source_external_id, title, content_markdown, content_plaintext, path_or_url, tags_json, checksum, metadata_json, created_at, updated_at FROM documents WHERE id = ?1";
        let mut stmt = connection.prepare(select)?;
        let mut rows = stmt.query_map(params![id], |row| {
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
        })?;

        if let Some(row) = rows.next() {
            Ok(Some(row?))
        } else {
            Ok(None)
        }
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
                "INSERT INTO chunks (id, document_id, ordinal, content, token_count, embedding_status, parent_id, summary, classification, metadata_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    chunk.id,
                    chunk.document_id,
                    chunk.ordinal,
                    chunk.content,
                    chunk.token_count,
                    chunk.embedding_status,
                    chunk.parent_id,
                    chunk.summary,
                    chunk.classification,
                    chunk.metadata_json,
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
            if chunk.embedding_status == "parent" {
                continue;
            }

            let fts_content = if let Some(ref sum) = chunk.summary {
                format!("{}\n\n{}", sum, chunk.content)
            } else {
                chunk.content.clone()
            };

            transaction.execute(
                "INSERT INTO chunk_fts (
                    chunk_id, document_id, source, title, content, tags, author, category, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    chunk.id,
                    document.id,
                    document.source_kind,
                    document.title,
                    fts_content,
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
            "SELECT id, document_id, ordinal, content, token_count, embedding_status, created_at, parent_id, summary, classification, metadata_json
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
                parent_id: row.get(7)?,
                summary: row.get(8)?,
                classification: row.get(9)?,
                metadata_json: row.get(10)?,
            })
        })?;
        Ok(rows.filter_map(Result::ok).collect())
    }

    #[allow(dead_code)]
    pub fn get_pending_chunks(&self) -> Result<Vec<DocumentChunk>> {
        let connection = self.connection.lock().expect("db lock poisoned");
        let mut statement = connection.prepare(
            "SELECT id, document_id, ordinal, content, token_count, embedding_status, created_at, parent_id, summary, classification, metadata_json
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
                parent_id: row.get(7)?,
                summary: row.get(8)?,
                classification: row.get(9)?,
                metadata_json: row.get(10)?,
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
                COALESCE(p.content, c.content) AS content,
                d.path_or_url,
                d.tags_json,
                d.metadata_json,
                d.created_at,
                d.updated_at,
                c.metadata_json AS chunk_metadata_json
             FROM chunks c
             JOIN documents d ON d.id = c.document_id
             LEFT JOIN chunks p ON p.id = c.parent_id
             WHERE c.id IN ({placeholders})"
        );
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map(rusqlite::params_from_iter(chunk_ids.iter()), |row| {
            let tags_json: String = row.get(7)?;
            let metadata_json: String = row.get(8)?;
            let metadata: serde_json::Value =
                serde_json::from_str(&metadata_json).unwrap_or(serde_json::json!({}));
            let chunk_metadata_json: Option<String> = row.get(11)?;
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
                chunk_metadata_json,
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
                d.updated_at,
                c.summary,
                c.metadata_json AS chunk_metadata_json
             FROM chunks c
             JOIN documents d ON d.id = c.document_id
             WHERE c.embedding_status != 'parent'
             ORDER BY d.id, c.ordinal ASC",
        )?;
        let rows = statement.query_map([], |row| {
            let tags_json: String = row.get(7)?;
            let metadata_json: String = row.get(8)?;
            let metadata: serde_json::Value =
                serde_json::from_str(&metadata_json).unwrap_or(serde_json::json!({}));
            
            let raw_content: String = row.get(5)?;
            let summary: Option<String> = row.get(11)?;
            let content = if let Some(sum) = summary {
                format!("{}\n\n{}", sum, raw_content)
            } else {
                raw_content
            };
            let chunk_metadata_json: Option<String> = row.get(12)?;

            Ok(ChunkSearchDocument {
                chunk_id: row.get(0)?,
                document_id: row.get(1)?,
                ordinal: row.get(2)?,
                source_kind: row.get(3)?,
                title: row.get(4)?,
                content,
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
                chunk_metadata_json,
            })
        })?;
        Ok(rows
            .filter_map(|r| match r {
                Ok(doc) => Some(doc),
                Err(e) => {
                    tracing::warn!("[DB] list_all_chunk_search_documents: skipped row due to error: {}", e);
                    None
                }
            })
            .collect())
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

    pub fn save_rag_telemetry(
        &self,
        query: &str,
        strategy: &str,
        confidence_score: u32,
        status: &str,
        reasons: &[String],
        confidence: &str,
        top_document: Option<&str>,
        top_document_score: Option<f32>,
        ambiguity_score: Option<f32>,
        hop_count: u32,
        retrieval_latency_ms: u32,
        rerank_latency_ms: u32,
        lineage_json: Option<&str>,
        // Recall evaluation fields
        pre_rerank_top_doc: Option<&str>,
        post_rerank_top_doc: Option<&str>,
        unique_docs_pre: u32,
        unique_docs_post: u32,
        top_doc_changed: bool,
        fact_coverage: f32,
    ) -> Result<()> {
        let connection = self.connection.lock().expect("db lock poisoned");
        let reasons_json = serde_json::to_string(reasons)?;
        let id = uuid::Uuid::new_v4().to_string();
        connection.execute(
            "INSERT INTO RAG_telemetry (
                id, query, strategy, confidence_score, status, reasons_json,
                confidence, top_document, top_document_score, ambiguity_score,
                hop_count, retrieval_latency_ms, rerank_latency_ms, lineage_json,
                pre_rerank_top_doc, post_rerank_top_doc, unique_docs_pre,
                unique_docs_post, top_doc_changed, fact_coverage
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                     ?15, ?16, ?17, ?18, ?19, ?20)",
            params![
                id,
                query,
                strategy,
                confidence_score,
                status,
                reasons_json,
                confidence,
                top_document,
                top_document_score,
                ambiguity_score,
                hop_count as i32,
                retrieval_latency_ms as i32,
                rerank_latency_ms as i32,
                lineage_json,
                pre_rerank_top_doc,
                post_rerank_top_doc,
                unique_docs_pre as i32,
                unique_docs_post as i32,
                top_doc_changed as i32,
                fact_coverage,
            ],
        )?;
        Ok(())
    }

    /// Returns a score distribution report from the last 100 queries stored in
    /// RAG_telemetry. Used to calibrate confidence thresholds from real data.
    ///
    /// Output includes:
    /// - `total_queries`: number of rows analysed
    /// - `status_counts`: map of status -> count
    /// - `top_doc_score_percentiles`: P10/P25/P50/P75/P90 of top_document_score
    /// - `confidence_score_percentiles`: P10/P25/P50/P75/P90 of confidence_score
    /// - `fact_coverage_percentiles`: P10/P25/P50/P75/P90 of fact_coverage
    /// - `top_doc_changed_rate`: fraction where top doc changed after reranking
    /// - `reranker_model`: always "cross-encoder/ms-marco-MiniLM-L-6-v2 (raw logits)"
    pub fn get_rag_performance_report(&self) -> anyhow::Result<serde_json::Value> {
        let connection = self.connection.lock().expect("db lock poisoned");

        let mut stmt = connection.prepare(
            "SELECT status, confidence_score, top_document_score, fact_coverage, top_doc_changed
             FROM RAG_telemetry
             ORDER BY rowid DESC
             LIMIT 100"
        )?;

        #[derive(Debug)]
        struct Row {
            status: String,
            confidence_score: f64,
            top_doc_score: Option<f64>,
            fact_coverage: Option<f64>,
            top_doc_changed: bool,
        }

        let rows: Vec<Row> = stmt.query_map([], |row| {
            Ok(Row {
                status: row.get::<_, String>(0)?,
                confidence_score: row.get::<_, f64>(1).unwrap_or(0.0),
                top_doc_score: row.get::<_, Option<f64>>(2)?,
                fact_coverage: row.get::<_, Option<f64>>(3)?,
                top_doc_changed: row.get::<_, i32>(4).map(|v| v != 0).unwrap_or(false),
            })
        })?.filter_map(|r| r.ok()).collect();

        let total = rows.len();

        let mut status_counts = std::collections::HashMap::<String, usize>::new();
        for r in &rows {
            *status_counts.entry(r.status.clone()).or_default() += 1;
        }

        let changed_count = rows.iter().filter(|r| r.top_doc_changed).count();
        let changed_rate = if total > 0 { changed_count as f64 / total as f64 } else { 0.0 };

        let percentiles = |mut values: Vec<f64>| -> serde_json::Value {
            if values.is_empty() {
                return serde_json::json!({ "p10": null, "p25": null, "p50": null, "p75": null, "p90": null });
            }
            values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let n = values.len();
            let at = |pct: f64| -> f64 {
                let idx = ((pct / 100.0) * (n - 1) as f64).round() as usize;
                values[idx.min(n - 1)]
            };
            serde_json::json!({ "p10": at(10.0), "p25": at(25.0), "p50": at(50.0), "p75": at(75.0), "p90": at(90.0) })
        };

        let conf_scores: Vec<f64> = rows.iter().map(|r| r.confidence_score).collect();
        let doc_scores: Vec<f64> = rows.iter().filter_map(|r| r.top_doc_score).collect();
        let cov_scores: Vec<f64> = rows.iter().filter_map(|r| r.fact_coverage).collect();

        Ok(serde_json::json!({
            "total_queries": total,
            "reranker_model": "cross-encoder/ms-marco-MiniLM-L-6-v2 (raw logits)",
            "status_counts": status_counts,
            "top_doc_changed_rate": changed_rate,
            "top_doc_changed_count": changed_count,
            "confidence_score_percentiles": percentiles(conf_scores),
            "top_doc_score_percentiles": percentiles(doc_scores),
            "fact_coverage_percentiles": percentiles(cov_scores),
            "threshold_reference": {
                "note": "Thresholds are sigmoid(logit). ms-marco logit range for correct matches: [-3, +4]",
                "empty_fires_below_sigmoid": 0.07,
                "low_fires_below_sigmoid": 0.12,
                "partial_fires_below_sigmoid": 0.30,
                "ok_fires_above_sigmoid": 0.30
            }
        }))
    }

    // ── Phase 2: Topic Cluster + Document Graph persistence ──────────────────

    /// Replaces all cluster assignments in `document_clusters` with the new assignments.
    /// Called at startup and after every sync to keep the table in sync with in-memory state.
    ///
    /// `assignments`: Vec of (document_id, cluster_id, confidence)
    pub fn save_document_clusters(&self, assignments: &[(String, String, f32)]) -> Result<()> {
        let connection = self.connection.lock().expect("db lock poisoned");
        connection.execute("DELETE FROM document_clusters", [])?;
        for (doc_id, cluster_id, confidence) in assignments {
            connection.execute(
                "INSERT OR REPLACE INTO document_clusters (document_id, cluster_id, confidence)
                 VALUES (?1, ?2, ?3)",
                params![doc_id, cluster_id, confidence],
            )?;
        }
        Ok(())
    }

    /// Returns all document IDs assigned to a given cluster, ordered by confidence desc.
    pub fn get_cluster_document_ids(&self, cluster_id: &str) -> Result<Vec<String>> {
        let connection = self.connection.lock().expect("db lock poisoned");
        let mut stmt = connection.prepare(
            "SELECT document_id FROM document_clusters
             WHERE cluster_id = ?1
             ORDER BY confidence DESC"
        )?;
        let ids = stmt.query_map(params![cluster_id], |row| row.get::<_, String>(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(ids)
    }

    /// Replaces all edges in `document_graph_edges` with the new edge set.
    /// Called at startup and after every sync.
    ///
    /// `edges`: Vec of (source_doc_id, target_doc_id, edge_type)
    pub fn save_document_graph_edges(&self, edges: &[(String, String, &str)]) -> Result<()> {
        let connection = self.connection.lock().expect("db lock poisoned");
        connection.execute("DELETE FROM document_graph_edges", [])?;
        for (source, target, edge_type) in edges {
            connection.execute(
                "INSERT OR REPLACE INTO document_graph_edges (source_doc_id, target_doc_id, edge_type)
                 VALUES (?1, ?2, ?3)",
                params![source, target, edge_type],
            )?;
        }
        Ok(())
    }
}


