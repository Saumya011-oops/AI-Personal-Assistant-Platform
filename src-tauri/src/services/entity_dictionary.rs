/// Entity Dictionary for query expansion, broad-topic detection, and recursive planning.
///
/// Merges compile-time static entity groups with dynamically discovered groups
/// from indexed document titles, tags, categories, and key metadata attributes.

use std::collections::HashSet;
use crate::domain::{ChunkSearchDocument, QueryAnalysis};

#[derive(Debug, Clone)]
pub struct EntityMatch {
    pub group_name: String,
    pub score: i32,
}

#[derive(Debug, Clone)]
pub struct EntityGroup {
    /// Cluster name (matches `document_clusters.cluster_id`)
    pub name: String,
    /// General topic nouns — trigger BroadTopic when matched as query subject
    pub primary_terms: Vec<String>,
    /// Specific proper nouns / tools — trigger FactLookup when matched as query subject
    pub specific_terms: Vec<String>,
    /// All terms used for query expansion (superset of primary + specific)
    pub expansion_terms: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct EntityDictionary {
    pub groups: Vec<EntityGroup>,
}

/// Helper function defining the static compile-time groups.
pub fn get_static_groups() -> Vec<EntityGroup> {
    vec![
        EntityGroup {
            name: "authentication".to_string(),
            primary_terms: vec!["authentication".to_string(), "auth".to_string(), "authorization".to_string()],
            specific_terms: vec![
                "oauth".to_string(), "pkce".to_string(), "jwt".to_string(), "sso".to_string(), "saml".to_string(),
                "openid".to_string(), "ldap".to_string(), "mfa".to_string(), "2fa".to_string(), "credential".to_string(),
                "credentials".to_string(), "keychain".to_string(), "keyring".to_string()
            ],
            expansion_terms: vec![
                "authentication".to_string(), "authorization".to_string(), "login".to_string(), "credential".to_string(),
                "oauth".to_string(), "pkce".to_string(), "jwt".to_string(), "token".to_string(), "sso".to_string(), "saml".to_string(), "session".to_string(),
                "access".to_string(), "identity".to_string(), "openid".to_string(), "ldap".to_string(), "mfa".to_string(),
            ],
        },
        EntityGroup {
            name: "monitoring".to_string(),
            primary_terms: vec!["monitoring".to_string(), "observability".to_string(), "telemetry".to_string()],
            specific_terms: vec!["prometheus".to_string(), "grafana".to_string(), "alertmanager".to_string(), "loki".to_string(), "jaeger".to_string(), "datadog".to_string()],
            expansion_terms: vec![
                "monitoring".to_string(), "observability".to_string(), "telemetry".to_string(), "metrics".to_string(),
                "logs".to_string(), "alerts".to_string(), "health".to_string(), "dashboard".to_string(),
                "prometheus".to_string(), "grafana".to_string(), "alertmanager".to_string(),
            ],
        },
        EntityGroup {
            name: "database".to_string(),
            primary_terms: vec!["database".to_string(), "storage".to_string(), "persistence".to_string()],
            specific_terms: vec!["qdrant".to_string(), "sqlite".to_string(), "postgres".to_string(), "postgresql".to_string(), "mysql".to_string(), "redis".to_string(), "mongodb".to_string()],
            expansion_terms: vec![
                "database".to_string(), "storage".to_string(), "persistence".to_string(), "vector".to_string(),
                "collection".to_string(), "index".to_string(), "query".to_string(), "retrieval".to_string(),
                "qdrant".to_string(), "sqlite".to_string(), "postgres".to_string(),
            ],
        },
        EntityGroup {
            name: "onboarding".to_string(),
            primary_terms: vec!["onboarding".to_string(), "orientation".to_string()],
            specific_terms: vec![],
            expansion_terms: vec![
                "onboarding".to_string(), "setup".to_string(), "guide".to_string(), "welcome".to_string(), "new".to_string(),
                "user".to_string(), "workflow".to_string(), "orientation".to_string(), "permissions".to_string(), "checklist".to_string(),
            ],
        },
        EntityGroup {
            name: "embedding".to_string(),
            primary_terms: vec!["embedding".to_string(), "embeddings".to_string(), "vectorization".to_string()],
            specific_terms: vec!["nomic".to_string(), "bge".to_string(), "e5".to_string(), "ada".to_string(), "text2vec".to_string()],
            expansion_terms: vec![
                "embedding".to_string(), "embeddings".to_string(), "vector".to_string(), "dimensions".to_string(),
                "model".to_string(), "chunking".to_string(), "similarity".to_string(), "dense".to_string(), "sparse".to_string(),
            ],
        },
        EntityGroup {
            name: "configuration".to_string(),
            primary_terms: vec!["configuration".to_string(), "settings".to_string(), "parameters".to_string()],
            specific_terms: vec![],
            expansion_terms: vec![
                "configuration".to_string(), "settings".to_string(), "parameters".to_string(), "chunk".to_string(), "size".to_string(),
                "overlap".to_string(), "threshold".to_string(), "timeout".to_string(), "limit".to_string(), "token".to_string(), "dimension".to_string(),
            ],
        },
        EntityGroup {
            name: "notion".to_string(),
            primary_terms: vec!["notion".to_string()],
            specific_terms: vec![],
            expansion_terms: vec![
                "notion".to_string(), "workspace".to_string(), "database".to_string(), "page".to_string(), "sync".to_string(), "api".to_string(), "integration".to_string(),
            ],
        },
        EntityGroup {
            name: "obsidian".to_string(),
            primary_terms: vec!["obsidian".to_string()],
            specific_terms: vec![],
            expansion_terms: vec![
                "obsidian".to_string(), "vault".to_string(), "vaults".to_string(), "local".to_string(), "folder".to_string(), "markdown".to_string(), "integration".to_string(),
            ],
        },
        EntityGroup {
            name: "rag".to_string(),
            primary_terms: vec!["rag".to_string(), "retrieval".to_string(), "retrieval-augmented".to_string()],
            specific_terms: vec!["bm25".to_string(), "rrf".to_string(), "mmr".to_string()],
            expansion_terms: vec![
                "rag".to_string(), "retrieval".to_string(), "augmented".to_string(), "generation".to_string(), "chunk".to_string(), "index".to_string(),
                "hybrid".to_string(), "dense".to_string(), "sparse".to_string(), "reranker".to_string(), "context".to_string(),
            ],
        },
    ]
}

pub fn is_valid_entity_term(term: &str) -> bool {
    // 1. Rejects purely numeric tokens
    if term.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }

    // 2. Rejects version patterns like v1, v2, v10
    if term.starts_with('v') && term.len() > 1 && term[1..].chars().all(|c| c.is_ascii_digit()) {
        return false;
    }

    // 3. Rejects roadmap, ticket, and internal prefixes
    let rejected_prefixes = ["rdmp", "prod", "fin", "hr", "strat", "roadmap", "ticket", "ref"];
    if rejected_prefixes.contains(&term) {
        return false;
    }

    // 4. Reject terms that contain both letters and numbers matching ticket/roadmap patterns (e.g. "strat001")
    for prefix in &rejected_prefixes {
        if term.starts_with(prefix) && term.len() > prefix.len() && term[prefix.len()..].chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
    }

    // 5. Reject hex/alphanumeric tokens of length >= 8 containing both letters and numbers (hash-like/ID-like chunk/document IDs)
    if term.len() >= 8 && term.chars().any(|c| c.is_ascii_digit()) && term.chars().any(|c| c.is_alphabetic()) {
        if term.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
            return false;
        }
    }

    true
}

impl EntityDictionary {
    /// Builds topic cluster assignments from a list of indexed documents.
    /// Incorporates the static compile-time groups and enriches them or constructs new
    /// groups dynamically using metadata-aware extraction.
    pub fn build(documents: &[ChunkSearchDocument]) -> Self {
        let mut groups = get_static_groups();

        let stop_words: HashSet<&str> = [
            "what", "is", "the", "does", "how", "to", "explain", "describe", "about",
            "system", "use", "connect", "with", "and", "or",
            "a", "an", "of", "which", "whose", "where", "whom", "related", "that",
            "this", "for", "from", "are", "was", "were", "has", "have", "had",
            "can", "will", "would", "should", "could", "may", "might", "in", "on",
            "guide", "flow", "overview", "detail", "details", "latest", "state", "sync",
            "pipeline", "process", "part", "some", "any",
            "who", "why", "we", "our", "us", "you", "your", "they", "them", "he", "she",
            "it", "its", "their", "his", "her",
        ].iter().cloned().collect();

        let clean_terms = |text: &str| -> Vec<String> {
            text.to_lowercase()
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty() && !stop_words.contains(s.as_str()) && s.len() > 2 && is_valid_entity_term(s))
                .collect()
        };

        // For each document, perform metadata-aware extraction of entities
        for doc in documents {
            let mut doc_entities = clean_terms(&doc.title);
            
            // 1. Tags metadata
            for tag in &doc.tags {
                let tag_lower = tag.to_lowercase();
                if is_valid_entity_term(&tag_lower) {
                    doc_entities.push(tag_lower);
                }
            }

            // 2. Category metadata
            if let Some(category) = &doc.category {
                let cat_lower = category.to_lowercase();
                if is_valid_entity_term(&cat_lower) {
                    doc_entities.push(cat_lower);
                }
            }

            // 4. Document metadata properties (e.g. technologies, systems, projects)
            if let serde_json::Value::Object(map) = &doc.metadata {
                for (key, val) in map {
                    let key_lower = key.to_lowercase();
                    if key_lower.contains("tech")
                        || key_lower.contains("tool")
                        || key_lower.contains("project")
                        || key_lower.contains("component")
                        || key_lower.contains("system")
                        || key_lower.contains("app")
                        || key_lower.contains("service")
                        || key_lower.contains("integration")
                    {
                        if let serde_json::Value::String(s) = val {
                            for term in clean_terms(s) {
                                doc_entities.push(term);
                            }
                        }
                    }
                }
            }

            doc_entities.sort();
            doc_entities.dedup();

            let mut matched_any_group = false;
            for group in &mut groups {
                if group.name == "notion" || group.name == "obsidian" {
                    continue;
                }

                let matches_group = group.name == doc.title.to_lowercase()
                    || group.primary_terms.iter().any(|t| doc_entities.contains(t) || doc.title.to_lowercase().contains(t));

                if matches_group {
                    matched_any_group = true;
                    // Enrich existing group
                    let generic_terms = ["setup", "integration", "integrations", "guide", "flow", "overview", "details", "process", "sync", "pipeline"];
                    for entity in &doc_entities {
                        if entity == "notion" || entity == "obsidian" {
                            continue;
                        }
                        if !group.primary_terms.contains(entity) && !group.specific_terms.contains(entity) {
                            if !generic_terms.contains(&entity.as_str()) {
                                group.specific_terms.push(entity.clone());
                            }
                        }
                        if !group.expansion_terms.contains(entity) {
                            group.expansion_terms.push(entity.clone());
                        }
                    }
                }
            }

            // If it didn't match any existing static group, build a new group dynamically
            if !matched_any_group && !doc_entities.is_empty() {
                // Reject generic terms from becoming group names
                let generic_names = ["setup", "integration", "integrations", "guide", "flow", "overview", "details", "process", "sync", "pipeline"];
                let group_name_opt = doc.tags.first()
                    .map(|t| t.to_lowercase())
                    .filter(|t| !generic_names.contains(&t.as_str()) && is_valid_entity_term(t))
                    .or_else(|| {
                        doc_entities.iter()
                            .find(|t| !generic_names.contains(&t.as_str()))
                            .cloned()
                    });

                if let Some(group_name) = group_name_opt {
                    if !groups.iter().any(|g| g.name == group_name) {
                        let primary_terms = vec![group_name.clone()];
                        let mut specific_terms = Vec::new();
                        let mut expansion_terms = doc_entities.clone();

                        for entity in &doc_entities {
                            if entity != &group_name && !generic_names.contains(&entity.as_str()) {
                                specific_terms.push(entity.clone());
                            }
                        }

                        for t in &primary_terms {
                            if !expansion_terms.contains(t) {
                                expansion_terms.push(t.clone());
                            }
                        }

                        groups.push(EntityGroup {
                            name: group_name,
                            primary_terms,
                            specific_terms,
                            expansion_terms,
                        });
                    }
                }
            }
        }

        EntityDictionary { groups }
    }

    pub fn score_group_for_query(&self, group: &EntityGroup, query: &str) -> i32 {
        let query_lower = query.to_lowercase();
        let mut score = 0;
        let name_lower = group.name.to_lowercase();
        if query_lower.contains(&name_lower) {
            score += 10;
        }
        for t in &group.primary_terms {
            if query_lower.contains(&t.to_lowercase()) {
                score += 5;
            }
        }
        for t in &group.specific_terms {
            if query_lower.contains(&t.to_lowercase()) {
                score += 3;
            }
        }
        for t in &group.expansion_terms {
            if query_lower.contains(&t.to_lowercase()) {
                score += 1;
            }
        }
        score
    }

    /// Returns all expansion terms for the query (union across all matched groups).
    pub fn expand(&self, query: &str) -> (String, Vec<String>) {
        let mut expansions: Vec<String> = Vec::new();
        let mut matched_groups: Vec<String> = Vec::new();

        for group in &self.groups {
            let score = self.score_group_for_query(group, query);
            if score >= 3 {
                matched_groups.push(group.name.clone());
                for term in &group.expansion_terms {
                    if !expansions.contains(term) {
                        expansions.push(term.clone());
                    }
                }
            }
        }

        let expanded = if expansions.is_empty() {
            query.to_string()
        } else {
            format!("{} {}", query, expansions.join(" "))
        };

        (expanded, matched_groups)
    }

    /// Detects all topic groups matching the query subject for ambiguity checks.
    pub fn detect_matching_groups(&self, query: &str) -> Vec<String> {
        let mut matches = Vec::new();
        if let Some(subject) = self.extract_subject(query) {
            let subject_lower = subject.to_lowercase();
            if self.is_specific_entity_query(&subject_lower) {
                return Vec::new();
            }
            for group in &self.groups {
                let matches_primary = group.name == subject_lower
                    || group.primary_terms.iter().any(|t| t == &subject_lower || subject_lower.contains(t))
                    || group.specific_terms.iter().any(|t| t == &subject_lower);
                if matches_primary {
                    matches.push(group.name.clone());
                }
            }
        }
        matches
    }

    /// Helper to strip broad intent prefixes and extract the query subject.
    pub fn extract_subject(&self, query: &str) -> Option<String> {
        let lower = query.to_lowercase();
        let lower = lower.trim();

        let broad_prefixes = [
            "explain ",
            "tell me about ",
            "overview of ",
            "describe ",
            "summarize ",
            "give me an overview of ",
            "what is the overview of ",
            "give an overview of ",
            "what is the meaning of ",
        ];

        let subject = broad_prefixes
            .iter()
            .find_map(|prefix| lower.strip_prefix(prefix))?
            .trim();

        if subject.is_empty() {
            None
        } else {
            Some(subject.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
        }
    }

    /// Detects whether the query is a BroadTopic.
    /// Returns `Some(cluster_name)` if exactly one topic cluster matches.
    pub fn detect_broad_topic(&self, query: &str) -> Option<String> {
        let matches = self.detect_matching_groups(query);
        if matches.len() == 1 {
            Some(matches[0].clone())
        } else {
            None
        }
    }

    /// Returns the entity group for the given cluster name.
    pub fn group_for_cluster(&self, cluster_name: &str) -> Option<&EntityGroup> {
        self.groups.iter().find(|g| g.name == cluster_name)
    }

    /// Returns true if the query subject matches a specific proper noun or tool.
    pub fn is_specific_entity_query(&self, query: &str) -> bool {
        let lower = query.to_lowercase();
        self.groups.iter().any(|g| {
            g.specific_terms.iter().any(|t| lower.contains(t))
        })
    }

    pub fn score_entities(&self, query: &str, analysis: &QueryAnalysis) -> Vec<EntityMatch> {
        const MAX_EXPANSION_SCORE: i32 = 6;
        let mut noun_phrases = HashSet::new();
        for entity in &analysis.entities {
            noun_phrases.insert(entity.to_lowercase());
        }

        let mut in_quotes = false;
        let mut current_quote_char = ' ';
        let mut current_phrase = String::new();
        for c in query.chars() {
            if in_quotes {
                if c == current_quote_char {
                    in_quotes = false;
                    let trimmed = current_phrase.trim();
                    if !trimmed.is_empty() {
                        noun_phrases.insert(trimmed.to_lowercase());
                    }
                    current_phrase.clear();
                } else {
                    current_phrase.push(c);
                }
            } else if c == '"' || c == '\'' || c == '`' {
                in_quotes = true;
                current_quote_char = c;
            }
        }

        let mut words = Vec::new();
        let mut current_word = String::new();
        for c in query.chars() {
            if c.is_alphanumeric() {
                current_word.push(c);
            } else {
                if !current_word.is_empty() {
                    words.push(current_word.clone());
                    current_word.clear();
                }
            }
        }
        if !current_word.is_empty() {
            words.push(current_word);
        }

        let mut temp_phrase = Vec::new();
        for w in &words {
            if let Some(first_char) = w.chars().next() {
                if first_char.is_uppercase() {
                    temp_phrase.push(w.clone());
                } else {
                    if !temp_phrase.is_empty() {
                        noun_phrases.insert(temp_phrase.join(" ").to_lowercase());
                        temp_phrase.clear();
                    }
                }
            }
        }
        if !temp_phrase.is_empty() {
            noun_phrases.insert(temp_phrase.join(" ").to_lowercase());
        }

        let query_lower = query.to_lowercase();
        let mut matches = Vec::new();

        for group in &self.groups {
            let mut score = 0;
            let group_name_lower = group.name.to_lowercase();

            if query_lower.contains(&group_name_lower) {
                score += 10;
            }

            let mut matched_primary = false;
            for t in &group.primary_terms {
                if query_lower.contains(&t.to_lowercase()) {
                    matched_primary = true;
                    break;
                }
            }
            if matched_primary {
                score += 6;
            }

            let mut matched_specific = false;
            for t in &group.specific_terms {
                if query_lower.contains(&t.to_lowercase()) {
                    matched_specific = true;
                    break;
                }
            }
            if matched_specific {
                score += 5;
            }

            let mut expansion_matches = 0;
            for t in &group.expansion_terms {
                if query_lower.contains(&t.to_lowercase()) {
                    expansion_matches += 1;
                }
            }
            let expansion_score = std::cmp::min(expansion_matches * 2, MAX_EXPANSION_SCORE);
            score += expansion_score;

            let mut all_group_terms = HashSet::new();
            all_group_terms.insert(group_name_lower.clone());
            for t in &group.primary_terms {
                all_group_terms.insert(t.to_lowercase());
            }
            for t in &group.specific_terms {
                all_group_terms.insert(t.to_lowercase());
            }
            for t in &group.expansion_terms {
                all_group_terms.insert(t.to_lowercase());
            }

            let mut match_count = 0;
            let mut total_occurrences = 0;
            for term in &all_group_terms {
                let term_matches = query_lower.matches(term).count();
                if term_matches > 0 {
                    match_count += 1;
                    total_occurrences += term_matches;
                }
            }
            if match_count >= 2 || total_occurrences >= 2 {
                score += 3;
            }

            let mut in_noun_phrases = false;
            for phrase in &noun_phrases {
                if phrase.contains(&group_name_lower) {
                    in_noun_phrases = true;
                    break;
                }
                for t in &group.primary_terms {
                    if phrase.contains(&t.to_lowercase()) {
                        in_noun_phrases = true;
                        break;
                    }
                }
                for t in &group.specific_terms {
                    if phrase.contains(&t.to_lowercase()) {
                        in_noun_phrases = true;
                        break;
                    }
                }
                if in_noun_phrases {
                    break;
                }
            }
            if in_noun_phrases {
                score += 5;
            }

            if score > 0 {
                matches.push(EntityMatch {
                    group_name: group.name.clone(),
                    score,
                });
            }
        }

        matches.sort_by(|a, b| b.score.cmp(&a.score));
        matches
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_entity_term() {
        // Valid terms
        assert!(is_valid_entity_term("authentication"));
        assert!(is_valid_entity_term("oauth2"));
        assert!(is_valid_entity_term("notion"));
        assert!(is_valid_entity_term("obsidian"));
        assert!(is_valid_entity_term("jwt"));
        assert!(is_valid_entity_term("prometheus"));
        assert!(is_valid_entity_term("grafana"));

        // Invalid: purely numeric
        assert!(!is_valid_entity_term("001"));
        assert!(!is_valid_entity_term("123456"));

        // Invalid: version patterns
        assert!(!is_valid_entity_term("v1"));
        assert!(!is_valid_entity_term("v2"));
        assert!(!is_valid_entity_term("v10"));

        // Invalid: roadmap/ticket prefixes
        assert!(!is_valid_entity_term("rdmp"));
        assert!(!is_valid_entity_term("strat"));
        assert!(!is_valid_entity_term("roadmap"));
        assert!(!is_valid_entity_term("ticket"));

        // Invalid: prefixes followed by digits
        assert!(!is_valid_entity_term("strat001"));
        assert!(!is_valid_entity_term("rdmp1234"));
        assert!(!is_valid_entity_term("roadmap99"));

        // Invalid: hash/ID-like (len >= 8, mixed letters and digits, only alphanum/dash/underscore)
        assert!(!is_valid_entity_term("doc12345678"));
        assert!(!is_valid_entity_term("chunk_12345678"));
        assert!(!is_valid_entity_term("hash-12ab34cd"));
    }
}
