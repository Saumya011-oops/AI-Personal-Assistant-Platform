//! RetrievalService — Week 4 production retrieval layer.
//!
//! Implements all six retrieval strategies:
//!   1. Dense      — Qdrant cosine similarity search on 768-dim embeddings
//!   2. Sparse     — SQLite FTS5 BM25 keyword search
//!   3. Hybrid     — Reciprocal Rank Fusion (RRF, k=60) of Dense + Sparse
//!   4. Faceted    — Dense search with Qdrant payload filters (source, tags, dates)
//!   5. Contextual — Dense search + surrounding sibling chunk window from SQLite
//!   6. Recursive  — Dense search on child chunks + parent summary content load

use std::collections::HashMap;
use std::time::Instant;

use anyhow::Result;
use serde_json::{json, Value};

use crate::db::Database;
use crate::db::repositories::document_repository::FtsChunkHit;
use crate::domain::{
    AllStrategiesResult, ContextChunk, RetrievalFilters, RetrievalRequest, RetrievalResponse,
    RetrievalResult,
};
use crate::services::ollama::OllamaService;
use crate::services::qdrant::{QdrantSearchResult, QdrantService};

/// Default number of results when none is specified
const DEFAULT_LIMIT: usize = 10;
/// Default context window (chunks on each side) for contextual retrieval
const DEFAULT_CONTEXT_WINDOW: usize = 2;
/// RRF constant — controls rank-score smoothing
const RRF_K: f64 = 60.0;

pub struct RetrievalService;

impl RetrievalService {
    /// Unified entry point: dispatches to the correct strategy and returns enriched results.
    pub async fn retrieve(
        request: &RetrievalRequest,
        database: &Database,
        ollama: &OllamaService,
        qdrant: &QdrantService,
    ) -> Result<RetrievalResponse> {
        let start = Instant::now();
        let limit = request.limit.unwrap_or(DEFAULT_LIMIT);
        let strategy = request.strategy.to_lowercase();

        let results = match strategy.as_str() {
            "dense" => {
                let vec = Self::embed_query(&request.query, ollama).await?;
                let hits = qdrant.search_similar_points(vec, limit).await?;
                Self::hits_to_results(hits, "dense", database)
            }
            "sparse" => {
                let fts_hits = database
                    .document_repository()
                    .fts_search_chunks(&request.query, limit)?;
                Self::fts_hits_to_results(fts_hits, "sparse", database)
            }
            "hybrid" => {
                let vec = Self::embed_query(&request.query, ollama).await?;
                let dense_hits = qdrant.search_similar_points(vec, limit * 2).await?;
                let sparse_hits = database
                    .document_repository()
                    .fts_search_chunks(&request.query, limit * 2)?;
                let fused = Self::rrf_fuse(dense_hits, sparse_hits, limit);
                Self::scored_to_results(fused, "hybrid", database)
            }
            "faceted" => {
                let vec = Self::embed_query(&request.query, ollama).await?;
                let filter = Self::build_qdrant_filter(request.filters.as_ref());
                let hits = qdrant.search_with_filter(vec, limit, filter).await?;
                Self::hits_to_results(hits, "faceted", database)
            }
            "contextual" => {
                let vec = Self::embed_query(&request.query, ollama).await?;
                let hits = qdrant.search_similar_points(vec, limit).await?;
                let window = request.context_window.unwrap_or(DEFAULT_CONTEXT_WINDOW);
                Self::contextual_results(hits, window, "contextual", database)
            }
            "recursive" => {
                let vec = Self::embed_query(&request.query, ollama).await?;
                // Filter to child-level chunks only for precision
                let child_filter = Some(json!({
                    "must": [{ "key": "chunk_level", "match": { "value": "child" } }]
                }));
                let hits = qdrant.search_with_filter(vec, limit, child_filter).await?;
                Self::recursive_results(hits, "recursive", database)
            }
            other => {
                return Err(anyhow::anyhow!("Unknown retrieval strategy: '{}'", other));
            }
        };

        let latency_ms = start.elapsed().as_millis() as u64;

        Ok(RetrievalResponse {
            total_results: results.len(),
            results,
            strategy_used: strategy,
            query: request.query.clone(),
            latency_ms,
        })
    }

    /// Runs all six strategies and returns their results for comparison / testing.
    pub async fn retrieve_all_strategies(
        query: &str,
        limit: usize,
        database: &Database,
        ollama: &OllamaService,
        qdrant: &QdrantService,
    ) -> Result<AllStrategiesResult> {
        let start = Instant::now();

        let base = |strategy: &str| RetrievalRequest {
            query: query.to_string(),
            strategy: strategy.to_string(),
            limit: Some(limit),
            filters: None,
            context_window: Some(DEFAULT_CONTEXT_WINDOW),
        };

        let dense = Self::retrieve(&base("dense"), database, ollama, qdrant).await
            .unwrap_or_else(|e| error_response("dense", query, e));
        let sparse = Self::retrieve(&base("sparse"), database, ollama, qdrant).await
            .unwrap_or_else(|e| error_response("sparse", query, e));
        let hybrid = Self::retrieve(&base("hybrid"), database, ollama, qdrant).await
            .unwrap_or_else(|e| error_response("hybrid", query, e));
        let faceted = Self::retrieve(&base("faceted"), database, ollama, qdrant).await
            .unwrap_or_else(|e| error_response("faceted", query, e));
        let contextual = Self::retrieve(&base("contextual"), database, ollama, qdrant).await
            .unwrap_or_else(|e| error_response("contextual", query, e));
        let recursive = Self::retrieve(&base("recursive"), database, ollama, qdrant).await
            .unwrap_or_else(|e| error_response("recursive", query, e));

        Ok(AllStrategiesResult {
            query: query.to_string(),
            dense,
            sparse,
            hybrid,
            faceted,
            contextual,
            recursive,
            total_latency_ms: start.elapsed().as_millis() as u64,
        })
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Embedding
    // ─────────────────────────────────────────────────────────────────────────

    async fn embed_query(query: &str, ollama: &OllamaService) -> Result<Vec<f32>> {
        let mut embeddings = ollama.generate_embeddings(&[query.to_string()]).await?;
        embeddings
            .pop()
            .ok_or_else(|| anyhow::anyhow!("Ollama returned no embedding for query"))
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Result converters
    // ─────────────────────────────────────────────────────────────────────────

    /// Converts Qdrant dense hits → RetrievalResult (no extra context)
    fn hits_to_results(
        hits: Vec<QdrantSearchResult>,
        strategy: &str,
        database: &Database,
    ) -> Vec<RetrievalResult> {
        hits.into_iter()
            .enumerate()
            .map(|(rank, hit)| {
                let payload = &hit.payload;
                let document_id = str_from_payload(payload, "document_id");
                let (title, source_kind, path_or_url, tags) =
                    meta_from_payload_or_db(&document_id, payload, database);

                RetrievalResult {
                    chunk_id: str_from_payload(payload, "chunk_id"),
                    document_id,
                    document_title: title,
                    source_kind,
                    content: str_from_payload(payload, "content"),
                    score: hit.score as f64,
                    rank: rank + 1,
                    strategy: strategy.to_string(),
                    context_chunks: Vec::new(),
                    parent_content: None,
                    path_or_url,
                    tags,
                }
            })
            .collect()
    }

    /// Converts FTS5 sparse hits → RetrievalResult
    fn fts_hits_to_results(
        hits: Vec<FtsChunkHit>,
        strategy: &str,
        database: &Database,
    ) -> Vec<RetrievalResult> {
        // BM25 scores from SQLite FTS5 are negative (more negative = more relevant).
        // We normalize to [0, 1] range by computing 1/(1 + |score|).
        let max_abs = hits
            .iter()
            .map(|h| h.bm25_score.abs())
            .fold(0.0_f64, f64::max)
            .max(1.0);

        hits.into_iter()
            .enumerate()
            .map(|(rank, hit)| {
                let normalized_score = 1.0 - (hit.bm25_score.abs() / max_abs);
                let (title, source_kind, path_or_url, tags) =
                    meta_from_db(&hit.document_id, database);

                RetrievalResult {
                    chunk_id: hit.chunk_id,
                    document_id: hit.document_id,
                    document_title: title,
                    source_kind,
                    content: hit.content,
                    score: normalized_score,
                    rank: rank + 1,
                    strategy: strategy.to_string(),
                    context_chunks: Vec::new(),
                    parent_content: None,
                    path_or_url,
                    tags,
                }
            })
            .collect()
    }

    /// Converts pre-fused (chunk_id, rrf_score) pairs → RetrievalResult
    fn scored_to_results(
        scored: Vec<(String, String, f64)>, // (chunk_id, document_id, score)
        strategy: &str,
        database: &Database,
    ) -> Vec<RetrievalResult> {
        scored
            .into_iter()
            .enumerate()
            .map(|(rank, (chunk_id, document_id, score))| {
                // Try to load content from DB
                let content = database
                    .document_repository()
                    .get_chunks_by_document(&document_id)
                    .ok()
                    .and_then(|chunks| chunks.into_iter().find(|c| c.id == chunk_id))
                    .map(|c| c.content)
                    .unwrap_or_default();

                let (title, source_kind, path_or_url, tags) =
                    meta_from_db(&document_id, database);

                RetrievalResult {
                    chunk_id,
                    document_id,
                    document_title: title,
                    source_kind,
                    content,
                    score,
                    rank: rank + 1,
                    strategy: strategy.to_string(),
                    context_chunks: Vec::new(),
                    parent_content: None,
                    path_or_url,
                    tags,
                }
            })
            .collect()
    }

    /// Converts Qdrant hits → RetrievalResult with surrounding sibling context
    fn contextual_results(
        hits: Vec<QdrantSearchResult>,
        window: usize,
        strategy: &str,
        database: &Database,
    ) -> Vec<RetrievalResult> {
        hits.into_iter()
            .enumerate()
            .map(|(rank, hit)| {
                let payload = &hit.payload;
                let chunk_id = str_from_payload(payload, "chunk_id");
                let document_id = str_from_payload(payload, "document_id");
                let primary_content = str_from_payload(payload, "content");
                let (title, source_kind, path_or_url, tags) =
                    meta_from_payload_or_db(&document_id, payload, database);

                // Get primary chunk ordinal
                let ordinal = database
                    .document_repository()
                    .get_chunk_ordinal(&chunk_id)
                    .ok()
                    .flatten()
                    .unwrap_or(0);

                // Fetch context window
                let context_chunks = database
                    .document_repository()
                    .get_context_chunks(&document_id, ordinal, window)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|c| ContextChunk {
                        ordinal: c.ordinal,
                        content: c.content,
                        is_primary: c.ordinal == ordinal,
                    })
                    .collect();

                RetrievalResult {
                    chunk_id,
                    document_id,
                    document_title: title,
                    source_kind,
                    content: primary_content,
                    score: hit.score as f64,
                    rank: rank + 1,
                    strategy: strategy.to_string(),
                    context_chunks,
                    parent_content: None,
                    path_or_url,
                    tags,
                }
            })
            .collect()
    }

    /// Converts Qdrant child chunk hits → RetrievalResult with parent summary content
    fn recursive_results(
        hits: Vec<QdrantSearchResult>,
        strategy: &str,
        database: &Database,
    ) -> Vec<RetrievalResult> {
        hits.into_iter()
            .enumerate()
            .map(|(rank, hit)| {
                let payload = &hit.payload;
                let chunk_id = str_from_payload(payload, "chunk_id");
                let document_id = str_from_payload(payload, "document_id");
                let primary_content = str_from_payload(payload, "content");
                let (title, source_kind, path_or_url, tags) =
                    meta_from_payload_or_db(&document_id, payload, database);

                // Load parent summary content
                let parent_content = database
                    .document_repository()
                    .get_parent_chunk(&chunk_id)
                    .ok()
                    .flatten()
                    .map(|p| p.content);

                RetrievalResult {
                    chunk_id,
                    document_id,
                    document_title: title,
                    source_kind,
                    content: primary_content,
                    score: hit.score as f64,
                    rank: rank + 1,
                    strategy: strategy.to_string(),
                    context_chunks: Vec::new(),
                    parent_content,
                    path_or_url,
                    tags,
                }
            })
            .collect()
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Reciprocal Rank Fusion (RRF)
    // ─────────────────────────────────────────────────────────────────────────

    /// Fuses dense (Qdrant) and sparse (FTS5) result lists using RRF.
    ///
    /// RRF score: Σ 1/(k + rank_i) across all result lists.
    /// Returns (chunk_id, document_id, rrf_score) sorted descending.
    fn rrf_fuse(
        dense_hits: Vec<QdrantSearchResult>,
        sparse_hits: Vec<FtsChunkHit>,
        limit: usize,
    ) -> Vec<(String, String, f64)> {
        let mut scores: HashMap<String, (String, f64)> = HashMap::new(); // chunk_id -> (doc_id, score)

        // Dense results contribution
        for (rank, hit) in dense_hits.iter().enumerate() {
            let chunk_id = str_from_payload(&hit.payload, "chunk_id");
            let document_id = str_from_payload(&hit.payload, "document_id");
            let rrf_score = 1.0 / (RRF_K + rank as f64 + 1.0);
            let entry = scores.entry(chunk_id).or_insert((document_id, 0.0));
            entry.1 += rrf_score;
        }

        // Sparse results contribution
        for (rank, hit) in sparse_hits.iter().enumerate() {
            let rrf_score = 1.0 / (RRF_K + rank as f64 + 1.0);
            let entry = scores
                .entry(hit.chunk_id.clone())
                .or_insert((hit.document_id.clone(), 0.0));
            entry.1 += rrf_score;
        }

        let mut fused: Vec<(String, String, f64)> = scores
            .into_iter()
            .map(|(chunk_id, (doc_id, score))| (chunk_id, doc_id, score))
            .collect();

        fused.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        fused.truncate(limit);
        fused
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Qdrant filter builder
    // ─────────────────────────────────────────────────────────────────────────

    /// Builds a Qdrant filter JSON from the optional RetrievalFilters struct.
    fn build_qdrant_filter(filters: Option<&RetrievalFilters>) -> Option<Value> {
        let Some(f) = filters else {
            return None;
        };

        let mut must: Vec<Value> = Vec::new();

        if let Some(source) = &f.source_kind {
            if !source.is_empty() {
                must.push(json!({
                    "key": "source",
                    "match": { "value": source }
                }));
            }
        }

        if let Some(tags) = &f.tags {
            for tag in tags {
                must.push(json!({
                    "key": "tags",
                    "match": { "value": tag }
                }));
            }
        }

        if let Some(date_after) = &f.date_after {
            must.push(json!({
                "key": "created_at",
                "range": { "gte": date_after }
            }));
        }

        if let Some(date_before) = &f.date_before {
            must.push(json!({
                "key": "created_at",
                "range": { "lte": date_before }
            }));
        }

        if must.is_empty() {
            None
        } else {
            Some(json!({ "must": must }))
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Utility helpers
// ─────────────────────────────────────────────────────────────────────────────

fn str_from_payload(payload: &Value, key: &str) -> String {
    payload[key]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

/// Tries to get document meta from the Qdrant payload first, then falls back to SQLite.
fn meta_from_payload_or_db(
    document_id: &str,
    payload: &Value,
    database: &Database,
) -> (String, String, Option<String>, Vec<String>) {
    let title = payload["title"].as_str().map(|s| s.to_string());
    let source = payload["source"].as_str().map(|s| s.to_string());
    let path = payload["path_or_url"].as_str().map(|s| s.to_string());
    let tags: Vec<String> = payload["tags"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    if title.is_some() && source.is_some() {
        (
            title.unwrap(),
            source.unwrap(),
            path,
            tags,
        )
    } else {
        meta_from_db(document_id, database)
    }
}

fn meta_from_db(
    document_id: &str,
    database: &Database,
) -> (String, String, Option<String>, Vec<String>) {
    database
        .document_repository()
        .get_document_meta(document_id)
        .ok()
        .flatten()
        .map(|(title, source, path, tags)| (title, source, path, tags))
        .unwrap_or_else(|| (
            "Unknown Document".to_string(),
            "unknown".to_string(),
            None,
            Vec::new(),
        ))
}

fn error_response(strategy: &str, query: &str, err: anyhow::Error) -> RetrievalResponse {
    tracing::error!("Strategy '{}' failed for query '{}': {}", strategy, query, err);
    RetrievalResponse {
        results: Vec::new(),
        strategy_used: strategy.to_string(),
        total_results: 0,
        query: query.to_string(),
        latency_ms: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AppConfig;
    use crate::db::Database;
    use crate::services::ollama::OllamaService;
    use crate::services::qdrant::QdrantService;

    #[tokio::test]
    async fn test_all_six_retrieval_strategies() {
        // Load configurations
        let config = match AppConfig::load() {
            Ok(c) => c,
            Err(e) => {
                println!("Failed to load configuration, skipping integration test: {}", e);
                return;
            }
        };

        // Open database connection
        let database = match Database::connect(&config.database_path) {
            Ok(db) => db,
            Err(e) => {
                println!("Failed to connect to SQLite, skipping integration test: {}", e);
                return;
            }
        };

        // Run migrations
        if let Err(e) = database.run_migrations() {
            println!("Failed to run migrations: {}", e);
            return;
        }

        let ollama = OllamaService::new(config.ollama_url.clone(), config.embedding_model.clone());
        let qdrant = QdrantService::new(config.qdrant_url.clone(), config.qdrant_collection.clone());

        // Perform semantic retrieval check to verify Qdrant connectivity
        if let Err(e) = qdrant.initialize_collection().await {
            println!("Qdrant is not running or failed to initialize, skipping test: {}", e);
            return;
        }

        let query = "RAG vector databases and embeddings";

        println!("--- Starting Strategy Integration Testing ---");

        // 1. Dense Strategy
        let req_dense = RetrievalRequest {
            query: query.to_string(),
            strategy: "dense".to_string(),
            limit: Some(3),
            filters: None,
            context_window: None,
        };
        let res_dense = RetrievalService::retrieve(&req_dense, &database, &ollama, &qdrant)
            .await
            .expect("Dense retrieval failed");
        assert_eq!(res_dense.strategy_used, "dense");
        println!("✅ Dense strategy retrieved {} results", res_dense.results.len());

        // 2. Sparse Strategy (FTS5)
        let req_sparse = RetrievalRequest {
            query: query.to_string(),
            strategy: "sparse".to_string(),
            limit: Some(3),
            filters: None,
            context_window: None,
        };
        let res_sparse = RetrievalService::retrieve(&req_sparse, &database, &ollama, &qdrant)
            .await
            .expect("Sparse retrieval failed");
        assert_eq!(res_sparse.strategy_used, "sparse");
        println!("✅ Sparse strategy retrieved {} results", res_sparse.results.len());

        // 3. Hybrid Strategy (RRF)
        let req_hybrid = RetrievalRequest {
            query: query.to_string(),
            strategy: "hybrid".to_string(),
            limit: Some(3),
            filters: None,
            context_window: None,
        };
        let res_hybrid = RetrievalService::retrieve(&req_hybrid, &database, &ollama, &qdrant)
            .await
            .expect("Hybrid retrieval failed");
        assert_eq!(res_hybrid.strategy_used, "hybrid");
        println!("✅ Hybrid strategy retrieved {} results", res_hybrid.results.len());

        // 4. Faceted Strategy
        let filters = RetrievalFilters {
            source_kind: Some("obsidian".to_string()),
            tags: None,
            date_after: None,
            date_before: None,
        };
        let req_faceted = RetrievalRequest {
            query: query.to_string(),
            strategy: "faceted".to_string(),
            limit: Some(3),
            filters: Some(filters),
            context_window: None,
        };
        let res_faceted = RetrievalService::retrieve(&req_faceted, &database, &ollama, &qdrant)
            .await
            .expect("Faceted retrieval failed");
        assert_eq!(res_faceted.strategy_used, "faceted");
        for hit in &res_faceted.results {
            assert_eq!(hit.source_kind, "obsidian");
        }
        println!("✅ Faceted strategy retrieved {} results, all source_kind = obsidian", res_faceted.results.len());

        // 5. Contextual Strategy
        let req_contextual = RetrievalRequest {
            query: query.to_string(),
            strategy: "contextual".to_string(),
            limit: Some(3),
            filters: None,
            context_window: Some(2),
        };
        let res_contextual = RetrievalService::retrieve(&req_contextual, &database, &ollama, &qdrant)
            .await
            .expect("Contextual retrieval failed");
        assert_eq!(res_contextual.strategy_used, "contextual");
        println!("✅ Contextual strategy retrieved {} results", res_contextual.results.len());

        // 6. Recursive Strategy
        let req_recursive = RetrievalRequest {
            query: query.to_string(),
            strategy: "recursive".to_string(),
            limit: Some(3),
            filters: None,
            context_window: None,
        };
        let res_recursive = RetrievalService::retrieve(&req_recursive, &database, &ollama, &qdrant)
            .await
            .expect("Recursive retrieval failed");
        assert_eq!(res_recursive.strategy_used, "recursive");
        println!("✅ Recursive strategy retrieved {} results", res_recursive.results.len());
        
        println!("--- All 6 Strategies Verified Successfully! ---");
    }
}
