/// evaluation/executor.rs
///
/// Pipeline Executor — runs a TestCase through the full assistant pipeline and
/// captures every intermediate stage into an ExecutionTrace.
///
/// Responsibilities:
///  - Seed and clean up MemoryFixtures for deterministic tests.
///  - Capture: query analysis, dense/sparse/reranked chunks, recalled memories,
///    assembled prompt, raw LLM output, citations, final answer, and per-stage
///    latency.
///  - Return an ExecutionTrace regardless of whether execution succeeded; errors
///    are stored in trace.error so the Evaluator can classify the failure cause.

use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use chrono::Utc;
use tracing::{info, warn};

use crate::db::Database;
use crate::domain::RetrievalMode;
use crate::services::intent_router::IntentRouter;
use crate::services::memory::{DbMemory, MemoryService};
use crate::services::ollama::OllamaService;
use crate::services::retrieval::RetrievalService;

use super::types::*;

// ──────────────────────────────────────────────────────────────────────────────
// Executor
// ──────────────────────────────────────────────────────────────────────────────

pub struct Executor {
    database: Database,
    retrieval_service: Arc<RetrievalService>,
    memory_service: Arc<MemoryService>,
    ollama_service: Arc<OllamaService>,
    intent_router: IntentRouter,
    eval_conversation_id: String,
}

impl Executor {
    pub fn new(
        database: Database,
        retrieval_service: Arc<RetrievalService>,
        memory_service: Arc<MemoryService>,
        ollama_service: Arc<OllamaService>,
    ) -> Self {
        Self {
            database,
            retrieval_service,
            memory_service,
            ollama_service,
            intent_router: IntentRouter::new(),
            eval_conversation_id: "qa-eval-session".to_string(),
        }
    }

    /// Ensure the evaluation conversation row exists.
    pub fn ensure_eval_conversation(&self) -> Result<()> {
        let conn = self.database.get_connection();
        let conn = conn.lock().expect("db lock poisoned");
        conn.execute(
            "INSERT OR IGNORE INTO chats (id, title) VALUES (?1, ?2)",
            rusqlite::params![&self.eval_conversation_id, "QA Evaluation Session"],
        )?;
        Ok(())
    }

    /// Run a single test case and return its execution trace.
    pub async fn run(&self, test: &TestCase) -> ExecutionTrace {
        let total_start = Instant::now();
        let mut latency = LatencyBreakdown::default();

        // ── 1. Seed memory fixtures ───────────────────────────────────────────
        let seeded_ids = if !test.memory_fixtures.is_empty() {
            match self.seed_memories(&test.memory_fixtures).await {
                Ok(ids) => ids,
                Err(e) => {
                    warn!("Failed to seed memories for test {}: {:?}", test.id, e);
                    vec![]
                }
            }
        } else {
            vec![]
        };

        // ── 2. Execute the query through the full pipeline ────────────────────
        let pipeline_start = Instant::now();
        let response_result = self
            .retrieval_service
            .ask_assistant_with_mode(
                &self.database,
                &self.memory_service,
                &test.query,
                &self.eval_conversation_id,
                &self.intent_router,
                RetrievalMode::Evaluation,
            )
            .await;
        latency.total_ms = pipeline_start.elapsed().as_millis() as u64;

        // ── 3. Build the trace ────────────────────────────────────────────────
        let trace = match response_result {
            Ok(response) => {
                let diagnostics = response.diagnostics.clone();
                let confidence = response.confidence.clone();

                // Extract pre/post rerank chunks from diagnostics if available
                let (pre_rerank_chunks, post_rerank_chunks) =
                    if let Some(ref diag) = diagnostics {
                        let pre = diag
                            .pre_rerank_chunks
                            .iter()
                            .map(|dc| crate::domain::RetrievedChunk {
                                chunk_id: dc.chunk_id.clone(),
                                document_id: String::new(),
                                source: String::new(),
                                document_title: dc.document_title.clone(),
                                content: String::new(),
                                score: dc.retrieval_score,
                                retrieval_score: Some(dc.retrieval_score),
                                dense_score: None,
                                sparse_score: None,
                                fused_score: None,
                                reranker_score: Some(dc.rerank_score),
                                final_score: Some(dc.rerank_score),
                                ordinal: 0,
                                path_or_url: None,
                                tags: vec![],
                                author: None,
                                category: None,
                                created_at: None,
                                modified_at: None,
                                metadata: serde_json::Value::Null,
                            })
                            .collect();
                        let post = diag
                            .post_rerank_chunks
                            .iter()
                            .map(|dc| crate::domain::RetrievedChunk {
                                chunk_id: dc.chunk_id.clone(),
                                document_id: String::new(),
                                source: String::new(),
                                document_title: dc.document_title.clone(),
                                content: String::new(),
                                score: dc.rerank_score,
                                retrieval_score: Some(dc.retrieval_score),
                                dense_score: None,
                                sparse_score: None,
                                fused_score: None,
                                reranker_score: Some(dc.rerank_score),
                                final_score: Some(dc.rerank_score),
                                ordinal: 0,
                                path_or_url: None,
                                tags: vec![],
                                author: None,
                                category: None,
                                created_at: None,
                                modified_at: None,
                                metadata: serde_json::Value::Null,
                            })
                            .collect();
                        (pre, post)
                    } else {
                        (vec![], vec![])
                    };

                // Retrieve the memories that were recalled during this query
                let mem_start = Instant::now();
                let recalled_memories = self
                    .memory_service
                    .retrieve_memories_for_query(&test.query, 10)
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(RankedMemorySnapshot::from)
                    .collect::<Vec<_>>();
                latency.memory_retrieval_ms = mem_start.elapsed().as_millis() as u64;

                ExecutionTrace {
                    test_id: test.id.clone(),
                    query: test.query.clone(),
                    query_analysis: None, // populated by retrieval service if exposed
                    expanded_query: diagnostics.as_ref().map(|d| d.query_expanded.clone()),
                    pre_rerank_chunks,
                    post_rerank_chunks,
                    recalled_memories,
                    prompt_assembled: response.assembled_prompt.clone().unwrap_or_default(),
                    llm_response: response.answer.clone(),
                    citations: response.citations.clone(),
                    final_answer: response.answer.clone(),
                    confidence,
                    diagnostics,
                    latency,
                    error: None,
                }
            }
            Err(e) => {
                warn!("Execution error for test {}: {:?}", test.id, e);
                ExecutionTrace {
                    test_id: test.id.clone(),
                    query: test.query.clone(),
                    query_analysis: None,
                    expanded_query: None,
                    pre_rerank_chunks: vec![],
                    post_rerank_chunks: vec![],
                    recalled_memories: vec![],
                    prompt_assembled: String::new(),
                    llm_response: String::new(),
                    citations: vec![],
                    final_answer: String::new(),
                    confidence: None,
                    diagnostics: None,
                    latency,
                    error: Some(e.to_string()),
                }
            }
        };

        // ── 4. Clean up seeded memories ───────────────────────────────────────
        if !seeded_ids.is_empty() {
            if let Err(e) = self.cleanup_seeded_memories(&seeded_ids) {
                warn!("Failed to clean up seeded memories for test {}: {:?}", test.id, e);
            }
        }

        info!(
            "[EXECUTOR] Test {} completed in {}ms (error: {})",
            test.id,
            total_start.elapsed().as_millis(),
            trace.error.is_some()
        );

        trace
    }

    // ── Memory seeding helpers ────────────────────────────────────────────────

    async fn seed_memories(&self, fixtures: &[MemoryFixture]) -> Result<Vec<String>> {
        let mut seeded_ids = Vec::new();
        let conn = self.database.get_connection();

        for fixture in fixtures {
            // Compute a simulated timestamp for the fixture
            let age_secs = (fixture.simulated_age_days * 86400.0) as i64;
            let simulated_time = Utc::now() - chrono::Duration::seconds(age_secs);
            let time_str = simulated_time.format("%Y-%m-%d %H:%M:%S").to_string();

            let memory = DbMemory {
                id: fixture.id.clone(),
                r#type: fixture.memory_type.clone(),
                content: fixture.content.clone(),
                embedding_model: "nomic-embed-text".to_string(),
                importance: fixture.importance,
                confidence: 0.9,
                access_count: 1,
                last_used: time_str.clone(),
                created_at: time_str.clone(),
                updated_at: time_str.clone(),
                source_conversation: Some(self.eval_conversation_id.clone()),
                status: "active".to_string(),
                deleted_at: None,
            };

            // Insert directly into SQLite (bypasses queue for deterministic timing)
            {
                let conn_guard = conn.lock().expect("db lock poisoned");
                conn_guard.execute(
                    "INSERT OR REPLACE INTO memories
                     (id, type, content, embedding_model, importance, confidence,
                      access_count, last_used, created_at, updated_at,
                      source_conversation, status, deleted_at)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
                    rusqlite::params![
                        memory.id,
                        memory.r#type,
                        memory.content,
                        memory.embedding_model,
                        memory.importance,
                        memory.confidence,
                        memory.access_count,
                        memory.last_used,
                        memory.created_at,
                        memory.updated_at,
                        memory.source_conversation,
                        memory.status,
                        memory.deleted_at,
                    ],
                )?;
            }

            // Compute real embedding via Ollama for accurate vector recall.
            // Must use the 'search_document:' prefix to match how the memory
            // retrieval query uses 'search_query:' — nomic-embed-text is
            // asymmetric and the prefix pair must be consistent.
            let prefixed_content = format!("search_document: {}", fixture.content);
            let embedding = self
                .ollama_service
                .generate_embeddings(&[prefixed_content])
                .await
                .ok()
                .and_then(|mut v| v.pop())
                .unwrap_or_else(|| vec![0.0f32; 768]);

            // Upsert real embedding to Qdrant for semantic retrieval
            let _ = self
                .memory_service
                .qdrant()
                .upsert_memory(
                    &fixture.id,
                    embedding,
                    &fixture.memory_type,
                    &fixture.content,
                    fixture.importance,
                )
                .await;

            seeded_ids.push(fixture.id.clone());
        }

        Ok(seeded_ids)
    }

    fn cleanup_seeded_memories(&self, ids: &[String]) -> Result<()> {
        let conn = self.database.get_connection();
        let conn_guard = conn.lock().expect("db lock poisoned");
        for id in ids {
            conn_guard.execute(
                "DELETE FROM memories WHERE id = ?1",
                rusqlite::params![id],
            )?;
        }
        Ok(())
    }
}
