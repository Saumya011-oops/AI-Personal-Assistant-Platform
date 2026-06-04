use std::collections::{HashMap, HashSet};

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::db::Database;
use crate::domain::{
    AssistantResponse, Citation, MetadataFilters, QueryAnalysis, RetrievalResponse,
    RetrievalStrategy, RetrievedChunk,
};
use crate::services::context_builder::{BuiltContext, ContextBuilder};
use crate::services::groq::GroqService;
use crate::services::ollama::OllamaService;
use crate::services::qdrant::{QdrantSearchFilter, QdrantSearchResult, QdrantService};
use crate::services::query_analyzer::QueryAnalyzerService;
use crate::services::reranker::RerankerService;
use crate::services::sparse::SparseRetrievalService;

#[derive(Clone)]
pub struct RetrievalService {
    ollama_service: OllamaService,
    qdrant_service: QdrantService,
    sparse_service: SparseRetrievalService,
    groq_service: GroqService,
    query_analyzer_service: QueryAnalyzerService,
    reranker_service: RerankerService,
    context_builder: ContextBuilder,
}

impl RetrievalService {
    pub fn new(
        ollama_service: OllamaService,
        qdrant_service: QdrantService,
        sparse_service: SparseRetrievalService,
        groq_service: GroqService,
        query_analyzer_service: QueryAnalyzerService,
        reranker_service: RerankerService,
        context_builder: ContextBuilder,
    ) -> Self {
        Self {
            ollama_service,
            qdrant_service,
            sparse_service,
            groq_service,
            query_analyzer_service,
            reranker_service,
            context_builder,
        }
    }

    pub async fn ask_assistant(&self, database: &Database, query: &str) -> Result<AssistantResponse> {
        let retrieval = self.retrieve_documents(database, query).await?;
        let reranked_chunks = self
            .reranker_service
            .rerank(query, retrieval.results.clone())
            .await?;
        let built_context = self.context_builder.build(reranked_chunks.clone());
        let answer = self.generate_answer(query, &retrieval.analysis, &built_context).await?;
        let citations = built_context
            .chunks
            .into_iter()
            .map(|chunk| Citation {
                source: chunk.source,
                document_id: chunk.document_id,
                chunk_id: chunk.chunk_id,
                score: chunk.score,
            })
            .collect();

        Ok(AssistantResponse { answer, citations })
    }

    pub async fn retrieve_documents(
        &self,
        database: &Database,
        query: &str,
    ) -> Result<RetrievalResponse> {
        let analysis = self.query_analyzer_service.analyze(database, query).await?;
        let results = self
            .retrieve_with_strategy(database, query, &analysis.strategy, &analysis, 0)
            .await?;

        Ok(RetrievalResponse {
            query: query.to_string(),
            strategy_used: analysis.strategy.clone(),
            total_results: results.len(),
            analysis,
            results,
        })
    }

    pub async fn retrieve_with_strategy(
        &self,
        database: &Database,
        query: &str,
        strategy: &RetrievalStrategy,
        analysis: &QueryAnalysis,
        depth: usize,
    ) -> Result<Vec<RetrievedChunk>> {
        match strategy {
            RetrievalStrategy::Dense => {
                self.retrieve_dense(database, query, analysis.metadata_filters.as_ref(), 20)
                    .await
            }
            RetrievalStrategy::Sparse => {
                self.retrieve_sparse(database, query, analysis.metadata_filters.as_ref(), 20)
                    .await
            }
            RetrievalStrategy::Hybrid => self.retrieve_hybrid(database, query, analysis, 30).await,
            RetrievalStrategy::Faceted => self.retrieve_faceted(database, query, analysis).await,
            RetrievalStrategy::Contextual => self.retrieve_contextual(database, query, analysis).await,
            RetrievalStrategy::Recursive => self.retrieve_recursive(database, query, analysis, depth).await,
        }
    }

    async fn retrieve_dense(
        &self,
        database: &Database,
        query: &str,
        filters: Option<&MetadataFilters>,
        limit: usize,
    ) -> Result<Vec<RetrievedChunk>> {
        let query_embeddings = self.ollama_service.generate_embeddings(&[query.to_string()]).await?;
        let Some(query_vector) = query_embeddings.into_iter().next() else {
            return Err(anyhow!("query embedding generation returned no vectors"));
        };

        let filter = filters.and_then(build_qdrant_filter);
        let results = self
            .qdrant_service
            .search_similar_points(query_vector, limit, filter)
            .await?;

        self.hydrate_qdrant_results(database, results)
    }

    async fn retrieve_sparse(
        &self,
        database: &Database,
        query: &str,
        filters: Option<&MetadataFilters>,
        limit: usize,
    ) -> Result<Vec<RetrievedChunk>> {
        let sparse_hits = self
            .sparse_service
            .search(query, filters, limit)
            .await?;
        let ordered_chunk_ids = sparse_hits
            .iter()
            .map(|hit| hit.chunk_id.clone())
            .collect::<Vec<_>>();
        let mut hydrated = hydrate_chunk_ids(database, &ordered_chunk_ids)?;
        let score_map = sparse_hits
            .into_iter()
            .map(|hit| (hit.chunk_id, hit.score))
            .collect::<HashMap<_, _>>();

        for chunk in &mut hydrated {
            chunk.score = score_map.get(&chunk.chunk_id).copied().unwrap_or_default();
        }

        sort_descending(&mut hydrated);
        Ok(hydrated)
    }

    async fn retrieve_hybrid(
        &self,
        database: &Database,
        query: &str,
        analysis: &QueryAnalysis,
        limit: usize,
    ) -> Result<Vec<RetrievedChunk>> {
        let dense = self
            .retrieve_dense(database, query, analysis.metadata_filters.as_ref(), 20)
            .await?;
        let sparse = self
            .retrieve_sparse(database, query, analysis.metadata_filters.as_ref(), 20)
            .await?;

        let mut scores = HashMap::<String, f32>::new();
        let mut chunk_map = HashMap::<String, RetrievedChunk>::new();

        for (rank, chunk) in dense.into_iter().enumerate() {
            let score = reciprocal_rank_fusion(rank + 1);
            *scores.entry(chunk.chunk_id.clone()).or_default() += score;
            chunk_map.entry(chunk.chunk_id.clone()).or_insert(chunk);
        }

        for (rank, chunk) in sparse.into_iter().enumerate() {
            let score = reciprocal_rank_fusion(rank + 1);
            *scores.entry(chunk.chunk_id.clone()).or_default() += score;
            chunk_map.entry(chunk.chunk_id.clone()).or_insert(chunk);
        }

        let mut merged = chunk_map
            .into_iter()
            .map(|(chunk_id, mut chunk)| {
                chunk.score = scores.get(&chunk_id).copied().unwrap_or_default();
                chunk
            })
            .collect::<Vec<_>>();

        sort_descending(&mut merged);
        merged.truncate(limit);
        Ok(merged)
    }

    async fn retrieve_faceted(
        &self,
        database: &Database,
        query: &str,
        analysis: &QueryAnalysis,
    ) -> Result<Vec<RetrievedChunk>> {
        let mut results = self.retrieve_hybrid(database, query, analysis, 30).await?;
        results.retain(|chunk| matches_filters(chunk, &analysis.metadata_filters));
        Ok(results)
    }

    async fn retrieve_contextual(
        &self,
        database: &Database,
        query: &str,
        analysis: &QueryAnalysis,
    ) -> Result<Vec<RetrievedChunk>> {
        let mut results = self.retrieve_hybrid(database, query, analysis, 30).await?;
        let now = Utc::now();

        for chunk in &mut results {
            if let Some(modified_at) = chunk.modified_at.as_deref() {
                if let Ok(parsed) = DateTime::parse_from_rfc3339(modified_at) {
                    let days = now
                        .signed_duration_since(parsed.with_timezone(&Utc))
                        .num_days()
                        .max(0) as i32;
                    chunk.score *= 0.95_f32.powi(days);
                }
            }

            if is_upcoming_event(&chunk.metadata) {
                chunk.score *= 2.0;
            }
        }

        sort_descending(&mut results);
        Ok(results)
    }

    async fn retrieve_recursive(
        &self,
        database: &Database,
        query: &str,
        analysis: &QueryAnalysis,
        depth: usize,
    ) -> Result<Vec<RetrievedChunk>> {
        let mut accumulated = self.retrieve_hybrid(database, query, analysis, 20).await?;
        let mut seen_queries = HashSet::from([query.to_string()]);
        let mut hop = depth;

        while hop < 3 {
            let follow_up_questions = self
                .generate_follow_up_questions(query, &accumulated)
                .await
                .unwrap_or_default();

            if follow_up_questions.is_empty() {
                break;
            }

            let mut found_new_query = false;
            for sub_query in follow_up_questions.into_iter().take(3) {
                if !seen_queries.insert(sub_query.clone()) {
                    continue;
                }
                found_new_query = true;
                let sub_results = self.retrieve_hybrid(database, &sub_query, analysis, 10).await?;
                accumulated.extend(sub_results);
            }

            if !found_new_query {
                break;
            }

            deduplicate_chunks(&mut accumulated);
            sort_descending(&mut accumulated);
            accumulated = self.reranker_service.rerank(query, accumulated).await?;
            hop += 1;
        }

        Ok(accumulated)
    }

    async fn generate_follow_up_questions(
        &self,
        query: &str,
        retrieved_chunks: &[RetrievedChunk],
    ) -> Result<Vec<String>> {
        let snippet = retrieved_chunks
            .iter()
            .take(6)
            .map(|chunk| format!("{}: {}", chunk.document_title, chunk.content))
            .collect::<Vec<_>>()
            .join("\n");

        let system_prompt = "Generate follow-up retrieval questions needed to answer the original user query. Return strict JSON in the shape {\"questions\": [\"...\"]}. Return at most 3 concise questions.";
        let user_prompt = format!(
            "Original query: {query}\nRetrieved evidence:\n{snippet}\nReturn strict JSON."
        );
        let value = self.groq_service.chat_json(system_prompt, &user_prompt).await?;
        Ok(value
            .get("questions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|question| !question.is_empty())
            .map(str::to_string)
            .collect())
    }

    async fn generate_answer(
        &self,
        query: &str,
        analysis: &QueryAnalysis,
        context: &BuiltContext,
    ) -> Result<String> {
        let system_prompt = "You are Assistant Core. Answer using only the supplied grounded context. If the context is insufficient, say so clearly. Do not invent citations. Keep the answer concise but helpful.";
        let user_prompt = format!(
            "User query: {query}\nStrategy: {:?}\nGrounded context:\n{}\nProvide the final answer only.",
            analysis.strategy,
            context.context_text
        );
        self.groq_service.chat_text(system_prompt, &user_prompt).await
    }

    pub async fn initialize(&self, database: &Database) -> Result<()> {
        self.ensure_groq_ready()?;
        self.sparse_service.initialize().await?;
        let documents = database.document_repository().list_all_chunk_search_documents()?;
        self.sparse_service.rebuild_index(&documents).await?;
        self.reranker_service.initialize().await?;
        Ok(())
    }

    pub async fn clear_sparse_index(&self) -> Result<()> {
        self.sparse_service.clear_index().await
    }

    fn hydrate_qdrant_results(
        &self,
        database: &Database,
        results: Vec<QdrantSearchResult>,
    ) -> Result<Vec<RetrievedChunk>> {
        let ordered_chunk_ids = results.iter().map(|result| result.id.clone()).collect::<Vec<_>>();
        let mut hydrated = hydrate_chunk_ids(database, &ordered_chunk_ids)?;
        let score_map = results
            .into_iter()
            .map(|result| (result.id, result.score))
            .collect::<HashMap<_, _>>();

        for chunk in &mut hydrated {
            chunk.score = score_map.get(&chunk.chunk_id).copied().unwrap_or_default();
        }
        sort_descending(&mut hydrated);
        Ok(hydrated)
    }
}

impl RetrievalService {
    fn ensure_groq_ready(&self) -> Result<()> {
        if self.groq_service.is_configured() {
            Ok(())
        } else {
            Err(anyhow!("GROQ_API_KEY is not configured"))
        }
    }
}

fn hydrate_chunk_ids(database: &Database, ordered_chunk_ids: &[String]) -> Result<Vec<RetrievedChunk>> {
    let rows = database
        .document_repository()
        .get_chunk_search_documents_by_ids(ordered_chunk_ids)?;
    let row_map = rows
        .into_iter()
        .map(|row| (row.chunk_id.clone(), row))
        .collect::<HashMap<_, _>>();

    let mut hydrated = Vec::new();
    for chunk_id in ordered_chunk_ids {
        if let Some(row) = row_map.get(chunk_id) {
            hydrated.push(RetrievedChunk {
                chunk_id: row.chunk_id.clone(),
                document_id: row.document_id.clone(),
                source: row.source_kind.clone(),
                document_title: row.title.clone(),
                content: row.content.clone(),
                score: 0.0,
                ordinal: row.ordinal,
                path_or_url: row.path_or_url.clone(),
                tags: row.tags.clone(),
                author: row.author.clone(),
                category: row.category.clone(),
                created_at: row.created_at.clone(),
                modified_at: row.updated_at.clone(),
                metadata: row.metadata.clone(),
            });
        }
    }

    Ok(hydrated)
}

fn reciprocal_rank_fusion(rank: usize) -> f32 {
    1.0 / (60.0 + rank as f32)
}

fn sort_descending(chunks: &mut [RetrievedChunk]) {
    chunks.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

fn deduplicate_chunks(chunks: &mut Vec<RetrievedChunk>) {
    let mut seen = HashSet::new();
    chunks.retain(|chunk| seen.insert(chunk.chunk_id.clone()));
}

fn build_qdrant_filter(filters: &MetadataFilters) -> Option<QdrantSearchFilter> {
    let mut must = Vec::new();

    if let Some(sources) = &filters.source {
        must.push(json!({
            "key": "source",
            "match": { "any": sources }
        }));
    }

    if let Some(authors) = &filters.author {
        let lowered: Vec<String> = authors.iter().map(|s| s.to_lowercase()).collect();
        must.push(json!({
            "key": "author",
            "match": { "any": lowered }
        }));
    }

    if let Some(tags) = &filters.tags {
        let lowered: Vec<String> = tags.iter().map(|s| s.to_lowercase()).collect();
        must.push(json!({
            "key": "tags",
            "match": { "any": lowered }
        }));
    }

    if let Some(categories) = &filters.category {
        let lowered: Vec<String> = categories.iter().map(|s| s.to_lowercase()).collect();
        must.push(json!({
            "key": "category",
            "match": { "any": lowered }
        }));
    }

    if let Some(date_range) = &filters.date_range {
        if date_range.from.is_some() || date_range.to.is_some() {
            let mut range = serde_json::Map::new();
            if let Some(from) = &date_range.from {
                range.insert("gte".to_string(), Value::String(from.clone()));
            }
            if let Some(to) = &date_range.to {
                range.insert("lte".to_string(), Value::String(to.clone()));
            }
            must.push(json!({
                "key": "modified_at",
                "range": Value::Object(range),
            }));
        }
    }

    if must.is_empty() {
        None
    } else {
        Some(QdrantSearchFilter {
            must,
            should: Vec::new(),
            must_not: Vec::new(),
        })
    }
}

fn matches_filters(chunk: &RetrievedChunk, filters: &MetadataFilters) -> bool {
    if let Some(sources) = &filters.source {
        if !sources
            .iter()
            .any(|source| source.eq_ignore_ascii_case(&chunk.source))
        {
            return false;
        }
    }

    if let Some(authors) = &filters.author {
        let Some(author) = &chunk.author else {
            return false;
        };
        if !authors
            .iter()
            .any(|candidate| author.to_lowercase().contains(&candidate.to_lowercase()))
        {
            return false;
        }
    }

    if let Some(tags) = &filters.tags {
        if !tags.iter().any(|required| {
            chunk
                .tags
                .iter()
                .any(|tag| tag.to_lowercase().contains(&required.to_lowercase()))
        }) {
            return false;
        }
    }

    if let Some(categories) = &filters.category {
        let Some(category) = &chunk.category else {
            return false;
        };
        if !categories.iter().any(|value| {
            category
                .to_lowercase()
                .contains(&value.to_lowercase())
        }) {
            return false;
        }
    }

    if let Some(date_range) = &filters.date_range {
        let date = chunk
            .modified_at
            .as_deref()
            .or(chunk.created_at.as_deref())
            .unwrap_or_default();
        if let Some(from) = &date_range.from {
            if date < from.as_str() {
                return false;
            }
        }
        if let Some(to) = &date_range.to {
            if date > to.as_str() {
                return false;
            }
        }
    }

    true
}

fn is_upcoming_event(metadata: &Value) -> bool {
    metadata
        .get("is_upcoming_event")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| {
            metadata
                .get("upcoming")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        })
}

trait MetadataFiltersExt {
    fn as_ref(&self) -> Option<&MetadataFilters>;
}

impl MetadataFiltersExt for MetadataFilters {
    fn as_ref(&self) -> Option<&MetadataFilters> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reciprocal_rank_fusion_uses_exact_formula() {
        let score = reciprocal_rank_fusion(1);
        assert!((score - (1.0 / 61.0)).abs() < f32::EPSILON);
    }
}
