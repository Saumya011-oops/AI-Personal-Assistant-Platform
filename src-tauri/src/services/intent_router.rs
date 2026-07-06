/// Intent Router — Stage 1 of the Memory-Aware Pipeline Refactor
///
/// Classifies every user query into one of five intents using deterministic
/// heuristic rules. No LLM calls are made in v1.
///
/// Classification order (priority, highest first):
///   1. NORMAL_CHAT   — pure greeting / social phrase with no fact content
///   2. MEMORY_RECALL — self-referential question ("What is my name?")
///   3. HYBRID_QUERY  — knowledge question that also references personal context
///   4. MEMORY_STORE  — declarative personal fact ("My name is…")
///   5. RAG_QUERY     — pure knowledge question (default)
///
/// Ordering rationale:
/// - MEMORY_RECALL before HYBRID: "What is my name?" has both a knowledge
///   opener ("what is") and a personal marker ("my"), but it's a pure recall
///   question — so RECALL wins.
/// - HYBRID before MEMORY_STORE: "Explain OAuth for my project" contains
///   personal markers ("my project") but is a knowledge+context question —
///   so HYBRID wins over the "my project" MEMORY_STORE trigger.

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// The five intent classes produced by the router.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntentClass {
    /// User is sharing a personal fact that should be remembered.
    /// e.g. "My name is Saumya.", "I prefer concise answers.", "I use Rust."
    MemoryStore,

    /// User is asking about something the assistant should remember.
    /// e.g. "What is my name?", "What project am I building?"
    MemoryRecall,

    /// Pure knowledge / document question with no personal context.
    /// e.g. "Explain OAuth.", "What is PKCE?"
    RagQuery,

    /// Knowledge question that also references the user's personal context.
    /// e.g. "Explain OAuth for my project.", "How should auth work in my app?"
    HybridQuery,

    /// Social greeting / acknowledgement with no retrievable information.
    /// e.g. "Hello", "Thanks", "Good morning", "Nice"
    NormalChat,
}

impl std::fmt::Display for IntentClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IntentClass::MemoryStore  => write!(f, "MEMORY_STORE"),
            IntentClass::MemoryRecall => write!(f, "MEMORY_RECALL"),
            IntentClass::RagQuery     => write!(f, "RAG_QUERY"),
            IntentClass::HybridQuery  => write!(f, "HYBRID_QUERY"),
            IntentClass::NormalChat   => write!(f, "NORMAL_CHAT"),
        }
    }
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
pub struct IntentRouter;

impl IntentRouter {
    pub fn new() -> Self {
        Self
    }

    /// Classify `query` into one of the five intent classes.
    pub fn classify(&self, query: &str) -> IntentClass {
        let q = query.trim();
        let lower = q.to_lowercase();

        // ── 1. NORMAL_CHAT ────────────────────────────────────────────────
        // Pure greetings with no information content.
        if self.is_normal_chat(&lower) {
            return IntentClass::NormalChat;
        }

        // ── 2. MEMORY_RECALL ──────────────────────────────────────────────
        // Self-referential questions checked BEFORE hybrid/store so that
        // "What is my name?" doesn't bleed into HYBRID (has "what is" AND "my").
        if self.is_memory_recall(&lower) {
            return IntentClass::MemoryRecall;
        }

        // ── 3. HYBRID_QUERY ───────────────────────────────────────────────
        // Knowledge question + personal context.
        // Checked BEFORE MEMORY_STORE so that "Explain OAuth for my project"
        // doesn't get caught by the "my project" MEMORY_STORE trigger.
        if self.is_hybrid_query(&lower) {
            return IntentClass::HybridQuery;
        }

        // ── 4. MEMORY_STORE ───────────────────────────────────────────────
        // Declarative personal fact.
        if self.is_memory_store(&lower) {
            return IntentClass::MemoryStore;
        }

        // ── 5. RAG_QUERY (default) ────────────────────────────────────────
        IntentClass::RagQuery
    }

    // -----------------------------------------------------------------------
    // Private classifiers
    // -----------------------------------------------------------------------

    /// Returns `true` for pure social exchanges that carry no retrievable
    /// information. We keep this list intentionally narrow: anything that
    /// might contain a personal fact falls through to MEMORY_STORE.
    fn is_normal_chat(&self, lower: &str) -> bool {
        let words: Vec<&str> = lower.split_whitespace().collect();
        // Longer messages almost always contain real content
        if words.len() > 5 {
            return false;
        }

        let greeting_words = [
            "hi", "hello", "hey", "hiya", "howdy",
            "thanks", "thank", "ty", "thx",
            "good", "morning", "afternoon", "evening", "night",
            "ok", "okay", "cool", "nice", "great", "awesome", "wow",
            "sure", "yep", "yes", "no", "nope",
            "bye", "goodbye", "cya", "later",
            "lol", "haha", "ha",
        ];

        // Every word (stripped of punctuation) must be a known greeting word
        let non_greeting: Vec<&str> = words.iter()
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()))
            .filter(|w| !w.is_empty() && !greeting_words.contains(w))
            .collect();

        non_greeting.is_empty()
    }

    /// Returns `true` for self-referential questions whose answer is
    /// expected to come from memory, not from documents.
    fn is_memory_recall(&self, lower: &str) -> bool {
        // These patterns are all question forms — they start with question
        // words or are questions about the user specifically.
        let patterns = [
            "what is my name",
            "what's my name",
            "who am i",
            "what am i building",
            "what am i working on",
            "what am i developing",
            "what am i creating",
            "what am i",
            "what are my",
            "what's my",
            "what is my",
            "what do i use",
            "what do i prefer",
            "what do i like",
            "what do i work on",
            "what do i",
            "which technologies do i",
            "which language do i",
            "which framework do i",
            "which tools do i",
            "what languages do i",
            "what frameworks do i",
            "what tools do i",
            "what project am i",
            "what do you know about me",
            "what do you remember",
            "what have you learned about me",
            "do you remember",
            "do you know my",
            "tell me what you know about me",
            "what have i told you",
            "what have i said",
        ];

        for pattern in &patterns {
            if lower.contains(pattern) {
                return true;
            }
        }

        // "about me" only as a standalone question probe, not mid-sentence
        if lower.starts_with("about me") || lower == "about me" {
            return true;
        }

        false
    }

    /// Returns `true` when a knowledge question explicitly references personal
    /// context, indicating both memory and RAG are needed.
    fn is_hybrid_query(&self, lower: &str) -> bool {
        // The query must contain BOTH a knowledge/technical opener AND a
        // personal context reference.

        // Knowledge / question openers (things that indicate external knowledge is needed)
        let knowledge_openers = [
            "explain ", "what is ", "what are ", "how does ", "how do ", "how to ",
            "how should ", "describe ", "overview of", "compare ", "difference between",
            "best practice", "recommend", "should i ", "should we ",
            "integrate ", "implement ", "configure ", "setup ",
        ];

        // Personal context phrases that indicate user-specific context is needed.
        // These are specific enough not to fire on pure memory questions.
        let personal_context_phrases = [
            " for my project", " for my app", " for my application",
            " for my stack", " for my use case", " for my setup",
            " for my system", " for my codebase", " for my workflow",
            " in my project", " in my app", " in my application",
            " in my stack", " in my system", " in my setup",
            " in my codebase",
            " with my project", " with my stack", " with my setup",
            " for me specifically", " in my context",
            " i'm building", " i am building",
            " i'm using", " i am using",
            " i'm working on", " i am working on",
        ];

        let has_knowledge = knowledge_openers.iter().any(|k| lower.contains(k));
        let has_personal  = personal_context_phrases.iter().any(|p| lower.contains(p));

        has_knowledge && has_personal
    }

    /// Returns `true` when the query is a declarative statement about the user
    /// that contains information worth storing as a memory.
    fn is_memory_store(&self, lower: &str) -> bool {
        // --- Sentence-opening declaratives ---
        // These patterns are anchored to the START of the sentence so that
        // "How should auth work in my app?" doesn't trigger on "my app".
        let declarative_prefixes = [
            "my name is",
            "my name's",
            "i am ",
            "i'm ",
            "i use ",
            "i used ",
            "i prefer ",
            "i like ",
            "i dislike ",
            "i hate ",
            "i love ",
            "i work ",
            "i work on",
            "i work at",
            "i switched",
            "i have switched",
            "i changed",
            "i just ",
            "i recently ",
            "i mainly ",
            "i mostly ",
            "i always ",
            "i usually ",
            "i typically ",
            "i generally ",
            "i tend to",
            "call me ",
            "it uses ",
            "it is ",
            "it runs ",
            "it's ",
        ];

        for prefix in &declarative_prefixes {
            if lower.starts_with(prefix) {
                // Exclude questions starting with these words (e.g. "Is it…")
                if !is_question(lower) {
                    return true;
                }
            }
        }

        // --- Explicit memory-store directives (can appear anywhere) ---
        let explicit_store_phrases = [
            "remember that",
            "remember this",
            "remember me as",
            "note that",
            "note this",
            "keep in mind that",
            "fyi,",
            "fyi:",
            "fyi -",
            "by the way,",
            "btw,",
            "btw:",
        ];

        for phrase in &explicit_store_phrases {
            if lower.contains(phrase) {
                return true;
            }
        }

        // --- Relocation / migration style ---
        if lower.contains(" switched from ") || lower.contains(" migrated from ")
            || lower.contains(" moved from ") || lower.contains(" migrated to ")
        {
            return true;
        }

        false
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns `true` if the text looks like a question.
fn is_question(lower: &str) -> bool {
    lower.ends_with('?')
        || lower.starts_with("what ")
        || lower.starts_with("which ")
        || lower.starts_with("who ")
        || lower.starts_with("where ")
        || lower.starts_with("when ")
        || lower.starts_with("why ")
        || lower.starts_with("how ")
        || lower.starts_with("is ")
        || lower.starts_with("are ")
        || lower.starts_with("do ")
        || lower.starts_with("does ")
        || lower.starts_with("can ")
        || lower.starts_with("could ")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn route(q: &str) -> IntentClass {
        IntentRouter::new().classify(q)
    }

    // NORMAL_CHAT
    #[test] fn greet_hello()       { assert_eq!(route("Hello"), IntentClass::NormalChat); }
    #[test] fn greet_thanks()      { assert_eq!(route("Thanks!"), IntentClass::NormalChat); }
    #[test] fn greet_good_morning(){ assert_eq!(route("Good morning"), IntentClass::NormalChat); }
    #[test] fn greet_nice()        { assert_eq!(route("Nice"), IntentClass::NormalChat); }
    #[test] fn greet_ok()          { assert_eq!(route("Ok cool"), IntentClass::NormalChat); }

    // MEMORY_STORE
    #[test] fn store_name()        { assert_eq!(route("My name is Saumya."), IntentClass::MemoryStore); }
    #[test] fn store_building()    { assert_eq!(route("I am building a desktop AI assistant."), IntentClass::MemoryStore); }
    #[test] fn store_uses()        { assert_eq!(route("It uses Rust and React."), IntentClass::MemoryStore); }
    #[test] fn store_switched()    { assert_eq!(route("Great, I switched from React to Vue today."), IntentClass::MemoryStore); }
    #[test] fn store_prefer()      { assert_eq!(route("I prefer concise answers."), IntentClass::MemoryStore); }
    #[test] fn store_remember()    { assert_eq!(route("Remember this: I use neovim."), IntentClass::MemoryStore); }

    // MEMORY_RECALL
    #[test] fn recall_name()       { assert_eq!(route("What is my name?"), IntentClass::MemoryRecall); }
    #[test] fn recall_project()    { assert_eq!(route("What project am I working on?"), IntentClass::MemoryRecall); }
    #[test] fn recall_techs()      { assert_eq!(route("Which technologies do I use?"), IntentClass::MemoryRecall); }
    #[test] fn recall_remember()   { assert_eq!(route("What do you remember about me?"), IntentClass::MemoryRecall); }
    #[test] fn recall_building()   { assert_eq!(route("What am I building?"), IntentClass::MemoryRecall); }

    // HYBRID_QUERY
    #[test] fn hybrid_oauth()      { assert_eq!(route("Explain OAuth for my project."), IntentClass::HybridQuery); }
    #[test] fn hybrid_auth()       { assert_eq!(route("How should authentication work in my app?"), IntentClass::HybridQuery); }

    // RAG_QUERY
    #[test] fn rag_oauth()         { assert_eq!(route("Explain OAuth."), IntentClass::RagQuery); }
    #[test] fn rag_pkce()          { assert_eq!(route("What is PKCE?"), IntentClass::RagQuery); }
    #[test] fn rag_compare()       { assert_eq!(route("Compare Notion and Obsidian."), IntentClass::RagQuery); }
}
