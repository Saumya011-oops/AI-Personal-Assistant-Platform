use std::collections::HashSet;

use crate::domain::RetrievedChunk;

#[derive(Clone)]
pub struct ContextBuilder;

#[derive(Clone)]
pub struct BuiltContext {
    pub context_text: String,
    pub chunks: Vec<RetrievedChunk>,
}

impl ContextBuilder {
    pub fn new() -> Self {
        Self
    }

    pub fn build(&self, chunks: Vec<RetrievedChunk>) -> BuiltContext {
        let mut chunks = chunks;
        chunks.truncate(10);
        let mut seen_chunk_ids = HashSet::new();
        let mut unique_chunks = Vec::new();

        for chunk in chunks {
            if seen_chunk_ids.insert(chunk.chunk_id.clone()) {
                unique_chunks.push(chunk);
            }
        }

        unique_chunks.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut merged_chunks = Vec::new();
        let mut current: Option<RetrievedChunk> = None;

        for chunk in unique_chunks {
            match current.as_mut() {
                Some(active)
                    if active.document_id == chunk.document_id && chunk.ordinal == active.ordinal + 1 =>
                {
                    active.content = format!("{}\n{}", active.content, chunk.content);
                    active.score = active.score.max(chunk.score);
                    active.ordinal = chunk.ordinal;
                }
                Some(_) => {
                    if let Some(finished) = current.take() {
                        merged_chunks.push(finished);
                    }
                    current = Some(chunk);
                }
                None => current = Some(chunk),
            }
        }

        if let Some(finished) = current {
            merged_chunks.push(finished);
        }

        let mut token_budget = 4000_usize;
        let mut selected_chunks = Vec::new();
        let mut sections = Vec::new();

        for chunk in merged_chunks {
            let section = format!(
                "[source={}; documentId={}; chunkId={}; score={:.4}; title={}]\n{}",
                chunk.source,
                chunk.document_id,
                chunk.chunk_id,
                chunk.score,
                chunk.document_title,
                chunk.content
            );
            let estimated_tokens = estimate_tokens(&section);
            if estimated_tokens > token_budget {
                continue;
            }
            token_budget -= estimated_tokens;
            sections.push(section);
            selected_chunks.push(chunk);
        }

        BuiltContext {
            context_text: sections.join("\n\n"),
            chunks: selected_chunks,
        }
    }
}

fn estimate_tokens(text: &str) -> usize {
    text.split_whitespace().count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn chunk(chunk_id: &str, ordinal: i64, content: &str) -> RetrievedChunk {
        RetrievedChunk {
            chunk_id: chunk_id.to_string(),
            document_id: "doc-1".to_string(),
            source: "notion".to_string(),
            document_title: "Doc".to_string(),
            content: content.to_string(),
            score: 0.9,
            retrieval_score: None,
            ordinal,
            path_or_url: None,
            tags: vec!["deployment".to_string()],
            author: Some("John".to_string()),
            category: Some("notes".to_string()),
            created_at: None,
            modified_at: None,
            metadata: json!({"author":"John"}),
        }
    }

    #[test]
    fn removes_duplicates_and_merges_adjacent_chunks() {
        let builder = ContextBuilder::new();
        let built = builder.build(vec![
            chunk("c1", 0, "first"),
            chunk("c1", 0, "first"),
            chunk("c2", 1, "second"),
        ]);

        assert_eq!(built.chunks.len(), 1);
        assert!(built.context_text.contains("first"));
        assert!(built.context_text.contains("second"));
    }
}
