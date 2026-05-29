use uuid::Uuid;
use crate::domain::DocumentChunk;

// ─────────────────────────────────────────────────────────────────────────────
// ParagraphChunker — original paragraph-based chunker (unchanged behaviour)
// ─────────────────────────────────────────────────────────────────────────────

pub struct ParagraphChunker;

impl ParagraphChunker {
    /// Estimates token count based on a simple word count approximation (words * 1.3)
    pub fn estimate_tokens(text: &str) -> usize {
        let words = text.split_whitespace().count();
        if words == 0 {
            return 0;
        }
        (words as f64 * 1.3).ceil() as usize
    }

    /// Chunks a plaintext document by grouping paragraphs together up to a token limit (e.g. 512 tokens)
    pub fn chunk_document(
        document_id: &str,
        content: &str,
        max_tokens: usize,
    ) -> Vec<DocumentChunk> {
        let mut chunks = Vec::new();
        if content.trim().is_empty() {
            return chunks;
        }

        // Split by standard double-newline first, with single newline as secondary
        let paragraphs: Vec<&str> = if content.contains("\n\n") {
            content.split("\n\n").map(|s| s.trim()).filter(|s| !s.is_empty()).collect()
        } else {
            content.split('\n').map(|s| s.trim()).filter(|s| !s.is_empty()).collect()
        };

        let mut current_paragraphs = Vec::new();
        let mut current_tokens = 0;
        let mut ordinal = 0_i64;

        for paragraph in paragraphs {
            let paragraph_tokens = Self::estimate_tokens(paragraph);

            // Handle the case where a single paragraph itself is larger than max_tokens
            if paragraph_tokens > max_tokens {
                // If we have some pending paragraphs, flush them first
                if !current_paragraphs.is_empty() {
                    let chunk_text = current_paragraphs.join("\n\n");
                    chunks.push(DocumentChunk {
                        id: Uuid::new_v4().to_string(),
                        document_id: document_id.to_string(),
                        ordinal,
                        content: chunk_text,
                        token_count: current_tokens as i64,
                        embedding_status: "pending".to_string(),
                        chunk_level: "standard".to_string(),
                        parent_chunk_id: None,
                        created_at: chrono::Utc::now().to_rfc3339(),
                    });
                    ordinal += 1;
                    current_paragraphs.clear();
                    current_tokens = 0;
                }

                // Split the giant paragraph into smaller sub-paragraphs (e.g. at sentence boundaries)
                let sentences: Vec<&str> = paragraph
                    .split(". ")
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .collect();

                let mut temp_sentences = Vec::new();
                let mut temp_tokens = 0;

                for sentence in sentences {
                    let sentence_text = if sentence.ends_with('.') {
                        sentence.to_string()
                    } else {
                        format!("{}.", sentence)
                    };
                    let sentence_tokens = Self::estimate_tokens(&sentence_text);

                    if temp_tokens + sentence_tokens > max_tokens {
                        if !temp_sentences.is_empty() {
                            let chunk_text = temp_sentences.join(" ");
                            chunks.push(DocumentChunk {
                                id: Uuid::new_v4().to_string(),
                                document_id: document_id.to_string(),
                                ordinal,
                                content: chunk_text,
                                token_count: temp_tokens as i64,
                                embedding_status: "pending".to_string(),
                                chunk_level: "standard".to_string(),
                                parent_chunk_id: None,
                                created_at: chrono::Utc::now().to_rfc3339(),
                            });
                            ordinal += 1;
                            temp_sentences.clear();
                            temp_tokens = 0;
                        }
                    }
                    temp_sentences.push(sentence_text);
                    temp_tokens += sentence_tokens;
                }

                if !temp_sentences.is_empty() {
                    let chunk_text = temp_sentences.join(" ");
                    chunks.push(DocumentChunk {
                        id: Uuid::new_v4().to_string(),
                        document_id: document_id.to_string(),
                        ordinal,
                        content: chunk_text,
                        token_count: temp_tokens as i64,
                        embedding_status: "pending".to_string(),
                        chunk_level: "standard".to_string(),
                        parent_chunk_id: None,
                        created_at: chrono::Utc::now().to_rfc3339(),
                    });
                    ordinal += 1;
                }
                continue;
            }

            // Normal case: group paragraphs
            if current_tokens + paragraph_tokens > max_tokens {
                // Flush current chunk
                let chunk_text = current_paragraphs.join("\n\n");
                chunks.push(DocumentChunk {
                    id: Uuid::new_v4().to_string(),
                    document_id: document_id.to_string(),
                    ordinal,
                    content: chunk_text,
                    token_count: current_tokens as i64,
                    embedding_status: "pending".to_string(),
                    chunk_level: "standard".to_string(),
                    parent_chunk_id: None,
                    created_at: chrono::Utc::now().to_rfc3339(),
                });
                ordinal += 1;
                current_paragraphs.clear();
                current_tokens = 0;
            }

            current_paragraphs.push(paragraph.to_string());
            current_tokens += paragraph_tokens;
        }

        // Flush any remaining paragraphs
        if !current_paragraphs.is_empty() {
            let chunk_text = current_paragraphs.join("\n\n");
            chunks.push(DocumentChunk {
                id: Uuid::new_v4().to_string(),
                document_id: document_id.to_string(),
                ordinal,
                content: chunk_text,
                token_count: current_tokens as i64,
                embedding_status: "pending".to_string(),
                chunk_level: "standard".to_string(),
                parent_chunk_id: None,
                created_at: chrono::Utc::now().to_rfc3339(),
            });
        }

        chunks
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RecursiveChunker — two-level hierarchy for Recursive Retrieval Strategy
//
// Creates:
//  • Parent chunks (1024 tokens) — summary-level, used for context
//  • Child chunks  (256 tokens)  — fine-grained, used for dense search
//
// Both are embedded into Qdrant. Dense search finds child chunks; retrieval
// then loads parent content for richer context.
// ─────────────────────────────────────────────────────────────────────────────

pub struct RecursiveChunker;

impl RecursiveChunker {
    /// Builds a two-level chunk hierarchy for a document.
    ///
    /// Returns all chunks in a flat `Vec` — parents first, followed by their
    /// children. The caller (pipeline) must persist them in a single transaction
    /// so that parent UUIDs exist before child foreign keys reference them.
    pub fn chunk_document_recursive(
        document_id: &str,
        content: &str,
        parent_max_tokens: usize,
        child_max_tokens: usize,
    ) -> Vec<DocumentChunk> {
        if content.trim().is_empty() {
            return Vec::new();
        }

        // Step 1: Split into large parent-level segments
        let parent_texts = Self::split_into_segments(content, parent_max_tokens);

        let mut all_chunks: Vec<DocumentChunk> = Vec::new();
        let mut global_ordinal = 0_i64;

        for parent_text in &parent_texts {
            let parent_id = Uuid::new_v4().to_string();
            let parent_token_count = ParagraphChunker::estimate_tokens(parent_text) as i64;

            // Create the parent chunk
            let parent_chunk = DocumentChunk {
                id: parent_id.clone(),
                document_id: document_id.to_string(),
                ordinal: global_ordinal,
                content: parent_text.clone(),
                token_count: parent_token_count,
                embedding_status: "pending".to_string(),
                chunk_level: "parent".to_string(),
                parent_chunk_id: None,
                created_at: chrono::Utc::now().to_rfc3339(),
            };
            all_chunks.push(parent_chunk);
            global_ordinal += 1;

            // Step 2: Split parent text into smaller child segments
            let child_texts = Self::split_into_segments(parent_text, child_max_tokens);

            for child_text in &child_texts {
                let child_token_count = ParagraphChunker::estimate_tokens(child_text) as i64;
                let child_chunk = DocumentChunk {
                    id: Uuid::new_v4().to_string(),
                    document_id: document_id.to_string(),
                    ordinal: global_ordinal,
                    content: child_text.clone(),
                    token_count: child_token_count,
                    embedding_status: "pending".to_string(),
                    chunk_level: "child".to_string(),
                    parent_chunk_id: Some(parent_id.clone()),
                    created_at: chrono::Utc::now().to_rfc3339(),
                };
                all_chunks.push(child_chunk);
                global_ordinal += 1;
            }
        }

        all_chunks
    }

    /// Splits a text block into segments, each up to `max_tokens` tokens.
    /// Groups paragraphs greedily; falls back to sentence splitting for giant paragraphs.
    fn split_into_segments(text: &str, max_tokens: usize) -> Vec<String> {
        let paragraphs: Vec<&str> = if text.contains("\n\n") {
            text.split("\n\n").map(|s| s.trim()).filter(|s| !s.is_empty()).collect()
        } else {
            text.split('\n').map(|s| s.trim()).filter(|s| !s.is_empty()).collect()
        };

        let mut segments: Vec<String> = Vec::new();
        let mut current: Vec<&str> = Vec::new();
        let mut current_tokens: usize = 0;

        for para in &paragraphs {
            let para_tokens = ParagraphChunker::estimate_tokens(para);

            if para_tokens > max_tokens {
                // Flush pending
                if !current.is_empty() {
                    segments.push(current.join("\n\n"));
                    current.clear();
                    current_tokens = 0;
                }
                // Sentence-split the oversized paragraph
                let sentences: Vec<&str> = para.split(". ").map(str::trim).filter(|s| !s.is_empty()).collect();
                let mut temp: Vec<String> = Vec::new();
                let mut temp_tokens: usize = 0;
                for sent in sentences {
                    let s = if sent.ends_with('.') { sent.to_string() } else { format!("{}.", sent) };
                    let st = ParagraphChunker::estimate_tokens(&s);
                    if temp_tokens + st > max_tokens && !temp.is_empty() {
                        segments.push(temp.join(" "));
                        temp.clear();
                        temp_tokens = 0;
                    }
                    temp.push(s);
                    temp_tokens += st;
                }
                if !temp.is_empty() {
                    segments.push(temp.join(" "));
                }
                continue;
            }

            if current_tokens + para_tokens > max_tokens {
                segments.push(current.join("\n\n"));
                current.clear();
                current_tokens = 0;
            }
            current.push(para);
            current_tokens += para_tokens;
        }

        if !current.is_empty() {
            segments.push(current.join("\n\n"));
        }

        segments
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens() {
        let text = "Hello world this is a test";
        assert_eq!(ParagraphChunker::estimate_tokens(text), 8); // 6 * 1.3 = 7.8 -> ceil is 8
    }

    #[test]
    fn test_chunk_document_basic() {
        let content = "Paragraph 1 is here.\n\nParagraph 2 is also here.";
        let chunks = ParagraphChunker::chunk_document("doc1", content, 10);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].content, "Paragraph 1 is here.");
        assert_eq!(chunks[1].content, "Paragraph 2 is also here.");
    }

    #[test]
    fn test_chunk_document_standard_level() {
        let content = "Hello world.\n\nAnother paragraph here.";
        let chunks = ParagraphChunker::chunk_document("doc1", content, 100);
        assert!(chunks.iter().all(|c| c.chunk_level == "standard"));
        assert!(chunks.iter().all(|c| c.parent_chunk_id.is_none()));
    }

    #[test]
    fn test_recursive_chunker_creates_hierarchy() {
        let content = (0..20)
            .map(|i| format!("This is paragraph number {} with enough words to reach a decent token count for testing purposes.", i))
            .collect::<Vec<_>>()
            .join("\n\n");

        let chunks = RecursiveChunker::chunk_document_recursive("doc_rec", &content, 300, 80);

        let parents: Vec<_> = chunks.iter().filter(|c| c.chunk_level == "parent").collect();
        let children: Vec<_> = chunks.iter().filter(|c| c.chunk_level == "child").collect();

        assert!(!parents.is_empty(), "Should have at least one parent chunk");
        assert!(!children.is_empty(), "Should have at least one child chunk");

        // Every child must reference a valid parent id
        let parent_ids: std::collections::HashSet<_> = parents.iter().map(|p| p.id.as_str()).collect();
        for child in &children {
            let pid = child.parent_chunk_id.as_deref().expect("child must have parent_chunk_id");
            assert!(parent_ids.contains(pid), "Child references unknown parent: {}", pid);
        }
    }

    #[test]
    fn test_recursive_chunker_empty() {
        let chunks = RecursiveChunker::chunk_document_recursive("doc_empty", "", 1024, 256);
        assert!(chunks.is_empty());
    }
}
