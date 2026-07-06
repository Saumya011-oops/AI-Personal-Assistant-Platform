/// Unified Prompt Builder — Stage 3 of the Memory-Aware Pipeline Refactor
///
/// Assembles the ordered prompt context that is sent to the LLM for every
/// execution path: RAG_QUERY, MEMORY_RECALL, HYBRID_QUERY, NORMAL_CHAT, and
/// broad-topic synthesis.
///
/// Ordering (matches spec):
///   1. System Prompt       (caller-supplied, intent-specific)
///   2. Conversation Summary
///   3. Relevant Long-Term Memories
///   4. Relevant Episodes
///   5. Recent Conversation Messages
///   6. Retrieved RAG Documents
///   7. Current User Message

use crate::services::memory::DbMessage;

/// Input bag for `PromptBuilder::build_user_prompt()`.
pub struct PromptContext<'a> {
    /// Optional rolling summary of the conversation so far.
    pub convo_summary: Option<&'a str>,
    /// Long-term memory strings (PROFILE, PREFERENCE, …).
    pub long_term_memories: &'a [String],
    /// Episodic memory strings (EPISODE type).
    pub episodic_memories: &'a [String],
    /// Recent conversation messages (last N).
    pub recent_messages: &'a [DbMessage],
    /// Structured text assembled from RAG documents (may be empty).
    pub rag_context_markdown: &'a str,
    /// The verbatim user query.
    pub query: &'a str,
}

/// Stateless builder — all methods are pure functions with no side effects.
#[derive(Clone, Default)]
pub struct PromptBuilder;

impl PromptBuilder {
    pub fn new() -> Self {
        Self
    }

    /// Build the user-role prompt from the supplied context bag.
    /// Returns a single string ready to be passed to `groq_service.chat_text()`.
    pub fn build_user_prompt(&self, ctx: &PromptContext<'_>) -> String {
        let mut parts: Vec<String> = Vec::new();

        // ── 2. Conversation Summary ────────────────────────────────────────
        if let Some(sum) = ctx.convo_summary {
            if !sum.trim().is_empty() {
                parts.push(format!("### Conversation Summary:\n{}", sum));
            }
        }

        // ── 3. Relevant Long-Term Memories ────────────────────────────────
        if !ctx.long_term_memories.is_empty() {
            let lt_str = ctx
                .long_term_memories
                .iter()
                .map(|m| format!("- {}", m))
                .collect::<Vec<_>>()
                .join("\n");
            parts.push(format!("### Relevant Long-Term Memories:\n{}", lt_str));
        }

        // ── 4. Relevant Episodes ──────────────────────────────────────────
        if !ctx.episodic_memories.is_empty() {
            let ep_str = ctx
                .episodic_memories
                .iter()
                .map(|e| format!("- {}", e))
                .collect::<Vec<_>>()
                .join("\n");
            parts.push(format!("### Relevant Recent Episodes:\n{}", ep_str));
        }

        // ── 5. Recent Conversation Messages ───────────────────────────────
        if !ctx.recent_messages.is_empty() {
            let recent_str = ctx
                .recent_messages
                .iter()
                .map(|m| format!("{}: {}", m.role, m.content))
                .collect::<Vec<_>>()
                .join("\n");
            parts.push(format!("### Recent Conversation Messages:\n{}", recent_str));
        }

        // ── 6. Retrieved RAG Documents ────────────────────────────────────
        if !ctx.rag_context_markdown.trim().is_empty() {
            parts.push(format!(
                "### Retrieved RAG Documents:\n{}",
                ctx.rag_context_markdown
            ));
        }

        // ── 7. Current User Message ───────────────────────────────────────
        parts.push(format!("### Current User Message:\nUser query: {}", ctx.query));

        parts.join("\n\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_sections_are_ordered() {
        let builder = PromptBuilder::new();
        let ctx = PromptContext {
            convo_summary: Some("We discussed Rust."),
            long_term_memories: &["User's name is Saumya.".to_string()],
            episodic_memories: &[],
            recent_messages: &[],
            rag_context_markdown: "OAuth is an authorization framework.",
            query: "Explain OAuth.",
        };
        let prompt = builder.build_user_prompt(&ctx);
        let summary_pos  = prompt.find("Conversation Summary").unwrap();
        let lt_mem_pos   = prompt.find("Long-Term Memories").unwrap();
        let rag_pos      = prompt.find("Retrieved RAG Documents").unwrap();
        let query_pos    = prompt.find("Current User Message").unwrap();
        assert!(summary_pos < lt_mem_pos);
        assert!(lt_mem_pos  < rag_pos);
        assert!(rag_pos     < query_pos);
    }

    #[test]
    fn empty_sections_are_omitted() {
        let builder = PromptBuilder::new();
        let ctx = PromptContext {
            convo_summary: None,
            long_term_memories: &[],
            episodic_memories: &[],
            recent_messages: &[],
            rag_context_markdown: "",
            query: "Hello",
        };
        let prompt = builder.build_user_prompt(&ctx);
        assert!(!prompt.contains("Conversation Summary"));
        assert!(!prompt.contains("Long-Term Memories"));
        assert!(prompt.contains("Current User Message"));
    }
}
