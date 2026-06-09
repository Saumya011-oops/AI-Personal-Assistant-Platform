use uuid::Uuid;
use crate::domain::DocumentChunk;

#[derive(Debug, Clone)]
pub struct ChunkerConfig {
    pub use_fallback: bool,
    pub parent_target: usize,
    pub parent_max: usize,
    pub child_target: usize,
    pub child_max: usize,
    pub overlap_target: usize,
}

impl Default for ChunkerConfig {
    fn default() -> Self {
        Self {
            use_fallback: false,
            parent_target: 2000,
            parent_max: 2500,
            child_target: 400,
            child_max: 500,
            overlap_target: 100,
        }
    }
}

#[derive(Debug, Clone)]
enum Block {
    Heading { level: usize, text: String },
    CodeBlock { language: Option<String>, code: String },
    Table { raw: String },
    Paragraph { text: String },
}

impl Block {
    fn to_text(&self) -> String {
        match self {
            Block::Heading { level, text } => format!("{} {}", "#".repeat(*level), text),
            Block::CodeBlock { language, code } => {
                let lang = language.as_deref().unwrap_or("");
                format!("```{}\n{}\n```", lang, code)
            }
            Block::Table { raw } => raw.clone(),
            Block::Paragraph { text } => text.clone(),
        }
    }

    fn estimate_tokens(&self) -> usize {
        ParagraphChunker::estimate_tokens(&self.to_text())
    }
}

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

    /// Primary entry point that maintains backward compatibility.
    pub fn chunk_document(
        document_id: &str,
        content: &str,
        max_tokens: usize,
    ) -> Vec<DocumentChunk> {
        let config = ChunkerConfig {
            use_fallback: false,
            parent_target: 2000,
            parent_max: 2500,
            child_target: max_tokens.saturating_sub(100).max(300),
            child_max: max_tokens,
            overlap_target: 100,
        };
        Self::chunk_document_with_config(document_id, content, &config, "Document")
    }

    /// Ingests documents with custom sizing config and file name for contextual metadata.
    pub fn chunk_document_with_config(
        document_id: &str,
        content: &str,
        config: &ChunkerConfig,
        file_name: &str,
    ) -> Vec<DocumentChunk> {
        if content.trim().is_empty() {
            return Vec::new();
        }

        if config.use_fallback {
            return Self::chunk_document_fallback(document_id, content, config.child_max);
        }

        // 1. Parse document into blocks
        let blocks = Self::parse_blocks(content);

        // 2. Group blocks into parent chunks
        let mut parent_groups = Vec::new();
        let mut current_parent_blocks = Vec::new();
        let mut current_parent_tokens = 0;
        let mut current_section = "".to_string();
        let mut current_subsection = "".to_string();

        for block in &blocks {
            let block_tokens = block.estimate_tokens();

            let should_flush_heading = match block {
                Block::Heading { level, .. } => {
                    *level <= 2 && current_parent_tokens >= 1000
                }
                _ => false,
            };

            let should_flush_size = current_parent_tokens + block_tokens > config.parent_max;

            if (should_flush_heading || should_flush_size) && !current_parent_blocks.is_empty() {
                parent_groups.push((
                    current_parent_blocks.clone(),
                    current_section.clone(),
                    current_subsection.clone(),
                ));
                current_parent_blocks.clear();
                current_parent_tokens = 0;
            }

            if let Block::Heading { level, text } = block {
                if *level == 1 {
                    current_section = text.clone();
                    current_subsection = "".to_string();
                } else if *level == 2 {
                    current_subsection = text.clone();
                }
            }

            current_parent_blocks.push(block.clone());
            current_parent_tokens += block_tokens;
        }

        if !current_parent_blocks.is_empty() {
            parent_groups.push((
                current_parent_blocks,
                current_section,
                current_subsection,
            ));
        }

        // 3. Process parent groups and slice them into child chunks
        let mut chunks = Vec::new();
        let mut ordinal = 0_i64;

        for (p_blocks, p_section, p_subsection) in parent_groups {
            let parent_id = Uuid::new_v4().to_string();
            let parent_content = p_blocks.iter().map(|b| b.to_text()).collect::<Vec<_>>().join("\n\n");
            let parent_tokens = Self::estimate_tokens(&parent_content);
            let parent_class = Self::classify_content(&parent_content);
            
            let parent_metadata = serde_json::json!({
                "file_name": file_name,
                "fileName": file_name,
                "section": p_section,
                "subsection": p_subsection,
                "chunk_type": "parent",
                "chunkType": "parent",
            });

            // Push parent chunk
            chunks.push(DocumentChunk {
                id: parent_id.clone(),
                document_id: document_id.to_string(),
                ordinal,
                content: parent_content,
                token_count: parent_tokens as i64,
                embedding_status: "parent".to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
                parent_id: None,
                summary: None,
                classification: Some(parent_class),
                metadata_json: Some(parent_metadata.to_string()),
            });
            ordinal += 1;

            // Generate child chunks
            let mut i = 0;
            while i < p_blocks.len() {
                let mut child_blocks = Vec::new();
                let mut child_tokens = 0;
                let mut j = i;

                while j < p_blocks.len() {
                    let block = &p_blocks[j];
                    let block_tokens = block.estimate_tokens();

                    if child_tokens + block_tokens > config.child_max {
                        if child_blocks.is_empty() {
                            match block {
                                Block::CodeBlock { .. } | Block::Table { .. } => {
                                    child_blocks.push(block.clone());
                                    child_tokens += block_tokens;
                                    j += 1;
                                }
                                Block::Heading { .. } | Block::Paragraph { .. } => {
                                    let sentence_chunks = Self::chunk_giant_paragraph(
                                        &block.to_text(),
                                        config,
                                        Some(&parent_id),
                                        document_id,
                                        &mut ordinal,
                                        file_name,
                                        &p_section,
                                        &p_subsection,
                                    );
                                    chunks.extend(sentence_chunks);
                                    j += 1;
                                }
                            }
                        }
                        break;
                    }

                    child_blocks.push(block.clone());
                    child_tokens += block_tokens;
                    j += 1;

                    if child_tokens >= config.child_target {
                        break;
                    }
                }

                if !child_blocks.is_empty() {
                    let child_content = child_blocks.iter().map(|b| b.to_text()).collect::<Vec<_>>().join("\n\n");
                    let classification = Self::classify_content(&child_content);
                    let summary = Self::generate_summary(&child_content, file_name, &p_section, &p_subsection);
                    
                    let child_metadata = serde_json::json!({
                        "file_name": file_name,
                        "fileName": file_name,
                        "section": p_section,
                        "subsection": p_subsection,
                        "chunk_type": "child",
                        "chunkType": "child",
                    });

                    chunks.push(DocumentChunk {
                        id: Uuid::new_v4().to_string(),
                        document_id: document_id.to_string(),
                        ordinal,
                        content: child_content,
                        token_count: child_tokens as i64,
                        embedding_status: "pending".to_string(),
                        created_at: chrono::Utc::now().to_rfc3339(),
                        parent_id: Some(parent_id.clone()),
                        summary: Some(summary),
                        classification: Some(classification),
                        metadata_json: Some(child_metadata.to_string()),
                    });
                    ordinal += 1;
                }

                // Backtrack sliding window for overlap
                let mut next_i = j;
                let mut overlap_tokens = 0;
                for k in (i..j).rev() {
                    let k_tokens = p_blocks[k].estimate_tokens();
                    if overlap_tokens + k_tokens > config.overlap_target {
                        break;
                    }
                    overlap_tokens += k_tokens;
                    next_i = k;
                }

                if next_i == i {
                    next_i = j;
                }
                i = next_i;
            }
        }

        chunks
    }

    fn parse_blocks(content: &str) -> Vec<Block> {
        let mut blocks = Vec::new();
        let mut current_paragraph = Vec::new();
        let mut in_code_block = false;
        let mut code_block_lang = None;
        let mut code_block_lines = Vec::new();
        let mut in_html_table = false;
        let mut html_table_lines = Vec::new();

        let mut lines = content.lines().peekable();

        let flush_paragraph = |para_lines: &mut Vec<&str>, blocks_vec: &mut Vec<Block>| {
            if !para_lines.is_empty() {
                blocks_vec.push(Block::Paragraph {
                    text: para_lines.join("\n"),
                });
                para_lines.clear();
            }
        };

        while let Some(line) = lines.next() {
            // 1. Code Block handling
            if in_code_block {
                code_block_lines.push(line);
                if line.trim().starts_with("```") {
                    blocks.push(Block::CodeBlock {
                        language: code_block_lang.clone(),
                        code: code_block_lines.join("\n"),
                    });
                    code_block_lines.clear();
                    code_block_lang = None;
                    in_code_block = false;
                }
                continue;
            }

            if line.trim().starts_with("```") {
                flush_paragraph(&mut current_paragraph, &mut blocks);
                in_code_block = true;
                let lang = line.trim().trim_start_matches('`').trim().to_string();
                code_block_lang = if lang.is_empty() { None } else { Some(lang) };
                code_block_lines.push(line);
                continue;
            }

            // 2. HTML Table handling
            if in_html_table {
                html_table_lines.push(line);
                if line.to_lowercase().contains("</table>") {
                    blocks.push(Block::Table {
                        raw: html_table_lines.join("\n"),
                    });
                    html_table_lines.clear();
                    in_html_table = false;
                }
                continue;
            }

            if line.trim().to_lowercase().starts_with("<table") {
                flush_paragraph(&mut current_paragraph, &mut blocks);
                in_html_table = true;
                html_table_lines.push(line);
                continue;
            }

            // 3. Heading handling
            if line.trim().starts_with('#') {
                let trimmed = line.trim();
                let hash_count = trimmed.chars().take_while(|&c| c == '#').count();
                if hash_count > 0 && hash_count <= 6 {
                    let rest = &trimmed[hash_count..];
                    if rest.starts_with(' ') || rest.is_empty() {
                        flush_paragraph(&mut current_paragraph, &mut blocks);
                        blocks.push(Block::Heading {
                            level: hash_count,
                            text: rest.trim().to_string(),
                        });
                        continue;
                    }
                }
            }

            // 4. Markdown Table handling
            if Self::is_markdown_table_line(line) {
                flush_paragraph(&mut current_paragraph, &mut blocks);
                let mut table_lines = vec![line];
                while let Some(next_line) = lines.peek() {
                    if Self::is_markdown_table_line(next_line) {
                        table_lines.push(lines.next().unwrap());
                    } else {
                        break;
                    }
                }
                blocks.push(Block::Table {
                    raw: table_lines.join("\n"),
                });
                continue;
            }

            // 5. Paragraph line handling
            if line.trim().is_empty() {
                flush_paragraph(&mut current_paragraph, &mut blocks);
            } else {
                current_paragraph.push(line);
            }
        }

        flush_paragraph(&mut current_paragraph, &mut blocks);
        blocks
    }

    fn is_markdown_table_line(line: &str) -> bool {
        let trimmed = line.trim();
        if !trimmed.contains('|') {
            return false;
        }
        if trimmed.chars().all(|c| c == '|' || c == '-' || c == ':' || c.is_whitespace()) {
            return true;
        }
        if trimmed.starts_with('|') || trimmed.ends_with('|') {
            return true;
        }
        trimmed.chars().filter(|&c| c == '|').count() >= 2
    }

    fn chunk_giant_paragraph(
        paragraph_text: &str,
        config: &ChunkerConfig,
        parent_id: Option<&str>,
        document_id: &str,
        ordinal_counter: &mut i64,
        file_name: &str,
        section: &str,
        subsection: &str,
    ) -> Vec<DocumentChunk> {
        let mut chunks = Vec::new();
        let sentences: Vec<&str> = paragraph_text
            .split(". ")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        let mut i = 0;
        while i < sentences.len() {
            let mut child_sentences = Vec::new();
            let mut child_tokens = 0;
            let mut j = i;

            while j < sentences.len() {
                let s = sentences[j];
                let s_text = if s.ends_with('.') {
                    s.to_string()
                } else {
                    format!("{}.", s)
                };
                let s_tokens = Self::estimate_tokens(&s_text);

                if child_tokens + s_tokens > config.child_max {
                    if child_sentences.is_empty() {
                        child_sentences.push(s_text);
                        child_tokens += s_tokens;
                        j += 1;
                    }
                    break;
                }

                child_sentences.push(s_text);
                child_tokens += s_tokens;
                j += 1;

                if child_tokens >= config.child_target {
                    break;
                }
            }

            let chunk_content = child_sentences.join(" ");
            let classification = Self::classify_content(&chunk_content);
            let summary = Self::generate_summary(&chunk_content, file_name, section, subsection);

            let metadata = serde_json::json!({
                "file_name": file_name,
                "fileName": file_name,
                "section": section,
                "subsection": subsection,
                "chunk_type": "child",
                "chunkType": "child",
            });

            chunks.push(DocumentChunk {
                id: Uuid::new_v4().to_string(),
                document_id: document_id.to_string(),
                ordinal: *ordinal_counter,
                content: chunk_content,
                token_count: child_tokens as i64,
                embedding_status: "pending".to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
                parent_id: parent_id.map(str::to_string),
                summary: Some(summary),
                classification: Some(classification),
                metadata_json: Some(metadata.to_string()),
            });
            *ordinal_counter += 1;

            // Slide sentence window
            let mut next_i = j;
            let mut overlap_tokens = 0;
            for k in (i..j).rev() {
                let s = sentences[k];
                let s_text = if s.ends_with('.') {
                    s.to_string()
                } else {
                    format!("{}.", s)
                };
                let k_tokens = Self::estimate_tokens(&s_text);
                if overlap_tokens + k_tokens > config.overlap_target {
                    break;
                }
                overlap_tokens += k_tokens;
                next_i = k;
            }

            if next_i == i {
                next_i = j;
            }
            i = next_i;
        }

        chunks
    }

    fn classify_content(content: &str) -> String {
        let lower = content.to_lowercase();

        let mut code_score = 0;
        if lower.contains("```") {
            code_score += 10;
        }
        for keyword in &[
            "fn ", "pub fn", "impl ", "struct ", "class ", "interface ", 
            "import ", "const ", "let ", "function ", "def ", "return ", 
            "async ", "await", "assert_eq"
        ] {
            if lower.contains(keyword) {
                code_score += 2;
            }
        }

        let mut api_score = 0;
        for keyword in &[
            "api", "endpoint", "http", "json", "request", "response", 
            "headers", "status code", "query params", "swagger", "openapi",
            "get /", "post /", "put /", "delete /", "api/v"
        ] {
            if lower.contains(keyword) {
                api_score += 2;
            }
        }

        let mut arch_score = 0;
        for keyword in &[
            "architecture", "system design", "database", "schema", 
            "diagram", "uml", "flowchart", "microservices", "infrastructure", 
            "kubernetes", "docker-compose", "data flow"
        ] {
            if lower.contains(keyword) {
                arch_score += 2;
            }
        }

        let mut req_score = 0;
        for keyword in &[
            "requirements", "user story", "prd", "spec", "acceptance criteria", 
            "scope of work", "backlog", "jira", "milestones", "deliverable"
        ] {
            if lower.contains(keyword) {
                req_score += 2;
            }
        }

        let mut meet_score = 0;
        for keyword in &[
            "meeting", "agenda", "attendees", "minutes", "sync notes", 
            "action items", "discussion points", "date:", "time:"
        ] {
            if lower.contains(keyword) {
                meet_score += 2;
            }
        }

        let mut max_score = 0;
        let mut classification = "general".to_string();

        if code_score > max_score {
            max_score = code_score;
            classification = "code".to_string();
        }
        if api_score > max_score {
            max_score = api_score;
            classification = "api".to_string();
        }
        if arch_score > max_score {
            max_score = arch_score;
            classification = "architecture".to_string();
        }
        if req_score > max_score {
            max_score = req_score;
            classification = "requirements".to_string();
        }
        if meet_score > max_score {
            max_score = meet_score;
            classification = "meeting_notes".to_string();
        }

        if max_score > 0 {
            classification
        } else {
            "general".to_string()
        }
    }

    fn generate_summary(
        content: &str,
        file_name: &str,
        section: &str,
        subsection: &str,
    ) -> String {
        let mut sentences = Vec::new();
        let trimmed = content.trim();

        let mut current = String::new();
        let mut chars = trimmed.chars().peekable();
        while let Some(c) = chars.next() {
            current.push(c);
            if c == '.' || c == '?' || c == '!' {
                if chars.peek().map_or(true, |next| next.is_whitespace()) {
                    let s = current.trim().to_string();
                    if !s.is_empty() {
                        sentences.push(s);
                    }
                    current.clear();
                    if sentences.len() >= 2 {
                        break;
                    }
                }
            }
        }
        if !current.trim().is_empty() && sentences.len() < 2 {
            sentences.push(current.trim().to_string());
        }

        let sentences_summary = sentences.join(" ");

        let mut details = Vec::new();
        if content.contains("```") {
            details.push("code snippets");
        }
        if content.contains('|') && content.contains("---") {
            details.push("data tables");
        }

        let detail_str = if details.is_empty() {
            "".to_string()
        } else {
            format!(" (contains {})", details.join(" and "))
        };

        let mut context_prefix = format!("File: {}", file_name);
        if !section.is_empty() {
            context_prefix.push_str(&format!(", Section: {}", section));
        }
        if !subsection.is_empty() {
            context_prefix.push_str(&format!(" > {}", subsection));
        }

        format!(
            "[{}] {}{}",
            context_prefix,
            if sentences_summary.is_empty() { "Overview of this section." } else { &sentences_summary },
            detail_str
        )
    }

    pub fn chunk_document_fallback(
        document_id: &str,
        content: &str,
        max_tokens: usize,
    ) -> Vec<DocumentChunk> {
        let paragraphs: Vec<&str> = if content.contains("\n\n") {
            content.split("\n\n").map(|s| s.trim()).filter(|s| !s.is_empty()).collect()
        } else {
            content.split('\n').map(|s| s.trim()).filter(|s| !s.is_empty()).collect()
        };

        let mut chunks = Vec::new();
        let mut current_paragraphs = Vec::new();
        let mut current_tokens = 0;
        let mut ordinal = 0_i64;

        let flush_chunk = |current_paragraphs: &mut Vec<String>, current_tokens: usize, ordinal: &mut i64, chunks: &mut Vec<DocumentChunk>| {
            let chunk_text = current_paragraphs.join("\n\n");
            let classification = Self::classify_content(&chunk_text);
            let summary = Self::generate_summary(&chunk_text, "Document", "", "");
            let metadata = serde_json::json!({
                "file_name": "Document",
                "fileName": "Document",
                "section": "",
                "subsection": "",
                "chunk_type": "standalone",
                "chunkType": "standalone",
            });
            chunks.push(DocumentChunk {
                id: Uuid::new_v4().to_string(),
                document_id: document_id.to_string(),
                ordinal: *ordinal,
                content: chunk_text,
                token_count: current_tokens as i64,
                embedding_status: "pending".to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
                parent_id: None,
                summary: Some(summary),
                classification: Some(classification),
                metadata_json: Some(metadata.to_string()),
            });
            *ordinal += 1;
            current_paragraphs.clear();
        };

        for paragraph in paragraphs {
            let paragraph_tokens = Self::estimate_tokens(paragraph);

            if paragraph_tokens > max_tokens {
                if !current_paragraphs.is_empty() {
                    flush_chunk(&mut current_paragraphs, current_tokens, &mut ordinal, &mut chunks);
                    current_tokens = 0;
                }

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
                            let classification = Self::classify_content(&chunk_text);
                            let summary = Self::generate_summary(&chunk_text, "Document", "", "");
                            let metadata = serde_json::json!({
                                "file_name": "Document",
                                "fileName": "Document",
                                "section": "",
                                "subsection": "",
                                "chunk_type": "standalone",
                                "chunkType": "standalone",
                            });
                            chunks.push(DocumentChunk {
                                id: Uuid::new_v4().to_string(),
                                document_id: document_id.to_string(),
                                ordinal,
                                content: chunk_text,
                                token_count: temp_tokens as i64,
                                embedding_status: "pending".to_string(),
                                created_at: chrono::Utc::now().to_rfc3339(),
                                parent_id: None,
                                summary: Some(summary),
                                classification: Some(classification),
                                metadata_json: Some(metadata.to_string()),
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
                    let classification = Self::classify_content(&chunk_text);
                    let summary = Self::generate_summary(&chunk_text, "Document", "", "");
                    let metadata = serde_json::json!({
                        "file_name": "Document",
                        "fileName": "Document",
                        "section": "",
                        "subsection": "",
                        "chunk_type": "standalone",
                        "chunkType": "standalone",
                    });
                    chunks.push(DocumentChunk {
                        id: Uuid::new_v4().to_string(),
                        document_id: document_id.to_string(),
                        ordinal,
                        content: chunk_text,
                        token_count: temp_tokens as i64,
                        embedding_status: "pending".to_string(),
                        created_at: chrono::Utc::now().to_rfc3339(),
                        parent_id: None,
                        summary: Some(summary),
                        classification: Some(classification),
                        metadata_json: Some(metadata.to_string()),
                    });
                    ordinal += 1;
                }
                continue;
            }

            if current_tokens + paragraph_tokens > max_tokens {
                flush_chunk(&mut current_paragraphs, current_tokens, &mut ordinal, &mut chunks);
                current_tokens = 0;
            }

            current_paragraphs.push(paragraph.to_string());
            current_tokens += paragraph_tokens;
        }

        if !current_paragraphs.is_empty() {
            flush_chunk(&mut current_paragraphs, current_tokens, &mut ordinal, &mut chunks);
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
        // Under parent-child configuration, it generates a parent chunk and child chunks.
        // So we should have at least 1 parent and some child chunks.
        assert!(chunks.len() >= 2);
        
        let parents: Vec<_> = chunks.iter().filter(|c| c.embedding_status == "parent").collect();
        let children: Vec<_> = chunks.iter().filter(|c| c.embedding_status == "pending").collect();
        
        assert_eq!(parents.len(), 1);
        assert!(!children.is_empty());
    }

    #[test]
    fn test_technical_document_chunking() {
        let content = r#"# Main Title

## Section 1: Architecture
This is a paragraph describing system architecture. It contains databases, modules, and service layers.

```rust
fn handle_data(input: String) -> Result<Data> {
    Ok(Data::new(input))
}
```

## Section 2: API Specifications
Here is a table showing the API endpoints.

| Method | Endpoint | Description |
|---|---|---|
| GET | /api/v1/health | Health check |
| POST | /api/v1/sync | Start synchronization |

And details about request payloads.
"#;
        let config = ChunkerConfig {
            use_fallback: false,
            parent_target: 200,
            parent_max: 300,
            child_target: 50,
            child_max: 100,
            overlap_target: 10,
        };

        let chunks = ParagraphChunker::chunk_document_with_config("doc2", content, &config, "architecture_spec.md");
        
        assert!(!chunks.is_empty());
        
        // Ensure parent chunk has parent embedding_status
        let parent_chunks: Vec<_> = chunks.iter().filter(|c| c.embedding_status == "parent").collect();
        assert!(!parent_chunks.is_empty());
        
        // Ensure child chunks point to their parents
        let child_chunks: Vec<_> = chunks.iter().filter(|c| c.embedding_status == "pending").collect();
        assert!(!child_chunks.is_empty());
        
        for child in &child_chunks {
            assert!(child.parent_id.is_some());
            let parent_exists = parent_chunks.iter().any(|p| Some(p.id.clone()) == child.parent_id);
            assert!(parent_exists);
            
            // Check metadata fields
            let metadata: serde_json::Value = serde_json::from_str(child.metadata_json.as_ref().unwrap()).unwrap();
            assert_eq!(metadata["fileName"], "architecture_spec.md");
            assert!(metadata["section"].as_str().is_some());
            assert_eq!(metadata["chunkType"], "child");
            
            // Check classification
            assert!(child.classification.is_some());
            let class = child.classification.as_ref().unwrap();
            assert!(class == "code" || class == "api" || class == "architecture" || class == "general");
            
            // Check summary is generated
            assert!(child.summary.is_some());
            assert!(child.summary.as_ref().unwrap().contains("architecture_spec.md"));
        }
    }
}
