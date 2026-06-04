use uuid::Uuid;
use crate::domain::DocumentChunk;

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
                created_at: chrono::Utc::now().to_rfc3339(),
            });
        }

        chunks
    }
}

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
}
