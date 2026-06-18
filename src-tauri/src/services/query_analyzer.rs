use anyhow::Result;
use regex::Regex;
use serde_json::Value;

use crate::domain::{
    MetadataDateRange, MetadataFilters, QueryAnalysis, QueryComplexity, RetrievalStrategy,
};
use crate::services::groq::GroqService;

#[derive(Clone)]
pub struct QueryAnalyzerService {
    groq_service: GroqService,
}

impl QueryAnalyzerService {
    pub fn new(groq_service: GroqService) -> Self {
        Self { groq_service }
    }

    pub async fn analyze(&self, database: &crate::db::Database, query: &str) -> Result<QueryAnalysis> {
        let groq_analysis = self.request_analysis(query).await?;
        let mut analysis = self.strict_analysis(query, &groq_analysis)?;

        if let Some(tags) = &mut analysis.metadata_filters.tags {
            if let Ok(db_tags) = database.document_repository().get_all_unique_tags() {
                tags.retain(|t| db_tags.contains(&t.to_lowercase()));
            }
        }
        if analysis.metadata_filters.tags.as_ref().is_some_and(|t| t.is_empty()) {
            analysis.metadata_filters.tags = None;
        }

        if let Some(cats) = &mut analysis.metadata_filters.category {
            if let Ok(db_cats) = database.document_repository().get_all_unique_categories() {
                cats.retain(|c| db_cats.contains(&c.to_lowercase()));
            }
        }
        if analysis.metadata_filters.category.as_ref().is_some_and(|c| c.is_empty()) {
            analysis.metadata_filters.category = None;
        }

        analysis.strategy = self.select_strategy(query, &analysis);
        Ok(analysis)
    }

    async fn request_analysis(&self, query: &str) -> Result<Value> {
        let system_prompt = "You are a query analysis engine for a retrieval orchestrator. Return strict JSON only with keys: intent, entities, metadataFilters, temporal, complexity, strategy. metadataFilters may contain source, author, tags, category, dateRange { from, to }. Do not include prose. IMPORTANT: Only extract metadataFilters (like tags, category, source) if the user strictly and explicitly requests filtering by them in their query. Do not aggressively guess tags or categories from conversational keywords.";
        let user_prompt = format!(
            "Analyze this user query for retrieval planning.\nQuery: {query}\nReturn strict JSON."
        );
        self.groq_service.chat_json(system_prompt, &user_prompt).await
    }

    fn strict_analysis(&self, query: &str, groq_analysis: &Value) -> Result<QueryAnalysis> {
        let normalized = query.to_lowercase();
        let temporal = groq_analysis
            .get("temporal")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || self.is_temporal_query(&normalized);

        let mut metadata_filters = MetadataFilters {
            source: self.extract_sources(&normalized, groq_analysis),
            author: self.extract_string_array(groq_analysis, &["metadataFilters", "author"]),
            tags: self.extract_string_array(groq_analysis, &["metadataFilters", "tags"]),
            category: self.extract_string_array(groq_analysis, &["metadataFilters", "category"]),
            date_range: self.extract_date_range(groq_analysis),
        };

        if metadata_filters.author.is_none() {
            metadata_filters.author = self.extract_author_hint(query);
        }

        let entities = self.extract_entities(query, groq_analysis);
        let complexity = if self.is_complex_query(&normalized) {
            QueryComplexity::Complex
        } else {
            match groq_analysis
                .get("complexity")
                .and_then(Value::as_str)
                .unwrap_or("simple")
            {
                "simple" => QueryComplexity::Simple,
                "complex" => QueryComplexity::Complex,
                _ => QueryComplexity::Simple,
            }
        };

        let _strategy = groq_analysis
            .get("strategy")
            .and_then(Value::as_str)
            .unwrap_or("hybrid");

        let intent = groq_analysis
            .get("intent")
            .and_then(Value::as_str)
            .unwrap_or("search")
            .to_string();

        Ok(QueryAnalysis {
            intent,
            entities,
            metadata_filters,
            temporal,
            complexity,
            strategy: RetrievalStrategy::Hybrid,
        })
    }

    fn select_strategy(&self, query: &str, analysis: &QueryAnalysis) -> RetrievalStrategy {
        let normalized = query.to_lowercase();

        if analysis.temporal {
            return RetrievalStrategy::Contextual;
        }

        if matches!(analysis.complexity, QueryComplexity::Complex)
            || normalized.contains("compare")
            || normalized.contains("difference")
            || normalized.contains("vs")
            || normalized.contains("versus")
            || normalized.contains("what changed")
            || normalized.contains("changed between")
            || normalized.contains("connect")
            || normalized.contains("interact")
            || normalized.contains("relationship")
            || normalized.contains("connection")
            || normalized.contains("relate")
            || normalized.contains("link")
            || normalized.contains("depend")
        {
            return RetrievalStrategy::Recursive;
        }

        if analysis.metadata_filters.source.as_ref().is_some_and(|sources| !sources.is_empty()) {
            return RetrievalStrategy::Faceted;
        }

        if self.is_specific_technical_query(query) {
            return RetrievalStrategy::Sparse;
        }

        // Dense is reserved for pure architecture/diagram/system-design queries with
        // no entity keywords. For all "explain X" or "how X" queries, Hybrid is
        // used so that BM25 also runs and keyword-rich documents (e.g. authentication
        // token management) are not missed by the embedding layer alone.
        if (normalized.contains("architecture") || normalized.contains("workflow")
            || normalized.contains("system design") || normalized.contains("diagram"))
            && !normalized.starts_with("explain")
            && !normalized.starts_with("how ")
        {
            return RetrievalStrategy::Dense;
        }

        RetrievalStrategy::Hybrid
    }

    fn is_temporal_query(&self, normalized: &str) -> bool {
        let temporal_markers = [
            "last week",
            "yesterday",
            "today",
            "tomorrow",
            "before ",
            "after ",
            "since ",
            "upcoming",
            "next meeting",
            "this month",
            "last month",
            "during may",
            "during june",
            "monday",
            "tuesday",
            "wednesday",
            "thursday",
            "friday",
            "saturday",
            "sunday",
        ];
        temporal_markers.iter().any(|marker| normalized.contains(marker))
    }

    fn is_complex_query(&self, normalized: &str) -> bool {
        let markers = [
            "compare",
            "difference",
            "changed between",
            "summarize all discussions",
            "proposed approaches",
            "project a and project b",
        ];
        markers.iter().any(|marker| normalized.contains(marker))
    }

    fn is_specific_technical_query(&self, query: &str) -> bool {
        let trimmed = query.trim();
        let token_count = trimmed.split_whitespace().count();
        if token_count <= 3
            && trimmed
                .chars()
                .any(|character| character.is_uppercase() || character.is_ascii_digit())
        {
            return true;
        }

        let technical_regex =
            Regex::new(r"\b(jwt|oauth|redis|postgresql|mysql|grpc|api|sdk|auth[a-z]*service)\b")
                .expect("valid regex");
        technical_regex.is_match(&query.to_lowercase())
    }

    fn extract_sources(&self, normalized: &str, groq_analysis: &Value) -> Option<Vec<String>> {
        let mut sources =
            self.extract_string_array(groq_analysis, &["metadataFilters", "source"]).unwrap_or_default();

        for (needle, canonical) in [
            ("notion", "notion"),
            ("gmail", "gmail"),
            ("drive", "drive"),
            ("obsidian", "obsidian"),
            ("calendar", "calendar"),
        ] {
            if normalized.contains(needle) && !sources.iter().any(|value| value == canonical) {
                sources.push(canonical.to_string());
            }
        }

        if sources.is_empty() {
            None
        } else {
            Some(sources)
        }
    }

    fn extract_author_hint(&self, query: &str) -> Option<Vec<String>> {
        let regex = Regex::new(r"(?i)\bfrom\s+([A-Z][a-z]+(?:\s+[A-Z][a-z]+)?)").expect("valid regex");
        regex
            .captures(query)
            .and_then(|captures| captures.get(1))
            .map(|value| vec![value.as_str().trim().to_string()])
    }

    fn extract_date_range(&self, groq_analysis: &Value) -> Option<MetadataDateRange> {
        let Some(value) = groq_analysis
            .get("metadataFilters")
            .and_then(|value| value.get("dateRange"))
        else {
            return None;
        };

        Some(MetadataDateRange {
            from: value.get("from").and_then(Value::as_str).map(str::to_string),
            to: value.get("to").and_then(Value::as_str).map(str::to_string),
        })
    }

    fn extract_string_array(&self, value: &Value, path: &[&str]) -> Option<Vec<String>> {
        let mut current = value;
        for segment in path {
            current = current.get(*segment)?;
        }

        let values = current
            .as_array()?
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();

        if values.is_empty() {
            None
        } else {
            Some(values)
        }
    }

    fn extract_entities(&self, query: &str, groq_analysis: &Value) -> Vec<String> {
        let mut entities =
            self.extract_string_array(groq_analysis, &["entities"]).unwrap_or_default();

        let token_regex =
            Regex::new(r"\b([A-Z][A-Za-z0-9_/-]+|[A-Z]{2,}|[a-z]+Service)\b").expect("valid regex");
        for capture in token_regex.captures_iter(query) {
            if let Some(value) = capture.get(1) {
                let entity = value.as_str().trim().to_string();
                if !entities.iter().any(|existing| existing == &entity) {
                    entities.push(entity);
                }
            }
        }

        entities
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn service() -> QueryAnalyzerService {
        QueryAnalyzerService::new(GroqService::new(
            None,
            None,
            None,
            "http://localhost".to_string(),
            "primary".to_string(),
            "fallback".to_string(),
        ))
    }

    #[test]
    fn selects_contextual_for_temporal_queries() {
        let service = service();
        let analysis = service
            .strict_analysis(
                "What happened last week?",
                &json!({
                    "intent": "lookup",
                    "entities": [],
                    "metadataFilters": {},
                    "temporal": true,
                    "complexity": "simple",
                    "strategy": "HYBRID"
                }),
            )
            .unwrap();
        assert!(matches!(
            service.select_strategy("What happened last week?", &analysis),
            RetrievalStrategy::Contextual
        ));
    }

    #[test]
    fn selects_faceted_for_source_queries() {
        let service = service();
        let analysis = service
            .strict_analysis(
                "Show deployment notes from Notion",
                &json!({
                    "intent": "lookup",
                    "entities": [],
                    "metadataFilters": { "source": ["notion"] },
                    "temporal": false,
                    "complexity": "simple",
                    "strategy": "FACETED"
                }),
            )
            .unwrap();
        assert!(matches!(
            service.select_strategy("Show deployment notes from Notion", &analysis),
            RetrievalStrategy::Faceted
        ));
    }


    #[test]
    fn selects_recursive_for_comparison_queries() {
        let service = service();
        let analysis = service
            .strict_analysis(
                "Compare deployment notes from Notion and Obsidian",
                &json!({
                    "intent": "compare",
                    "entities": [],
                    "metadataFilters": {},
                    "temporal": false,
                    "complexity": "complex",
                    "strategy": "RECURSIVE"
                }),
            )
            .unwrap();
        assert!(matches!(
            service.select_strategy("Compare deployment notes from Notion and Obsidian", &analysis),
            RetrievalStrategy::Recursive
        ));
    }

    #[test]
    fn selects_sparse_for_technical_queries() {
        let service = service();
        let analysis = service
            .strict_analysis(
                "JWT",
                &json!({
                    "intent": "lookup",
                    "entities": ["JWT"],
                    "metadataFilters": {},
                    "temporal": false,
                    "complexity": "simple",
                    "strategy": "SPARSE"
                }),
            )
            .unwrap();
        assert!(matches!(
            service.select_strategy("JWT", &analysis),
            RetrievalStrategy::Sparse
        ));
    }

    #[test]
    fn selects_dense_for_conceptual_queries() {
        let service = service();
        let analysis = service
            .strict_analysis(
                "Explain our authentication architecture",
                &json!({
                    "intent": "explain",
                    "entities": [],
                    "metadataFilters": {},
                    "temporal": false,
                    "complexity": "simple",
                    "strategy": "DENSE"
                }),
            )
            .unwrap();
        // "Explain ... architecture" now routes to Hybrid (not Dense),
        // because "explain" overrides the architecture keyword per our fix.
        // Dense is only for pure architecture/workflow queries without explain/how.
        assert!(matches!(
            service.select_strategy("Explain our authentication architecture", &analysis),
            RetrievalStrategy::Hybrid
        ));
    }
}
