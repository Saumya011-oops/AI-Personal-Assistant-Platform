/// Document Graph for structural graph traversal in recursive retrieval.
///
/// The graph is an in-memory adjacency list built from all indexed documents.
/// Edges are extracted deterministically from document content at build time:
///   - `[[wikilink]]` patterns
///   - Markdown relative links `[text](../source/title.md)`
///   - Verbatim title mentions in content (titles with length > 8 chars)
///
/// In recursive retrieval, instead of running noisy inline keyword searches
/// for Hop 2, we simply look up graph neighbors of the Hop-1 document IDs.
/// This is faster, deterministic, and avoids the false-positive chains that
/// came from matching short title substrings.

use std::collections::{HashMap, HashSet};

use crate::domain::ChunkSearchDocument;

#[derive(Debug, Clone, Default)]
pub struct DocumentGraph {
    /// Outbound edges: source_doc_id → Vec<target_doc_id>
    edges: HashMap<String, Vec<String>>,
    /// All document titles for reverse lookup (title_lowercase → doc_id)
    title_index: HashMap<String, String>,
}

impl DocumentGraph {
    /// Builds the graph from a flat list of `ChunkSearchDocument`.
    ///
    /// Multiple chunks from the same document are deduped — we only need
    /// the document-level edges.
    pub fn build(documents: &[ChunkSearchDocument]) -> Self {
        let mut graph = DocumentGraph::default();

        // Build title index first (title lowercased, underscores→spaces, len > 8)
        let mut seen_docs: HashMap<String, String> = HashMap::new(); // doc_id → title
        for doc in documents {
            seen_docs
                .entry(doc.document_id.clone())
                .or_insert_with(|| doc.title.clone());
        }

        for (doc_id, title) in &seen_docs {
            let lower = title.to_lowercase().replace('_', " ");
            graph.title_index.insert(lower.clone(), doc_id.clone());
            // Also index with underscores for wikilink matching
            let underscored = title.to_lowercase().replace(' ', "_");
            if underscored != lower {
                graph.title_index.insert(underscored, doc_id.clone());
            }
            // Also index with dashes
            let dashed = title.to_lowercase().replace('_', "-");
            if dashed != lower {
                graph.title_index.insert(dashed, doc_id.clone());
            }
        }

        // Extract edges from chunk content
        // Compile regex patterns once
        let wikilink_re = regex::Regex::new(r"\[\[([a-zA-Z0-9_\-\s:]+)\]\]").unwrap();
        let md_link_re  = regex::Regex::new(r"\[[^\]]+\]\(\.\./[^/]+/([a-zA-Z0-9_\-]+)\.md\)").unwrap();

        // Group content by document ID
        let mut doc_content: HashMap<String, String> = HashMap::new();
        for doc in documents {
            doc_content
                .entry(doc.document_id.clone())
                .and_modify(|c| {
                    c.push(' ');
                    c.push_str(&doc.content);
                })
                .or_insert_with(|| doc.content.clone());
        }

        let mut edges_to_add = Vec::new();

        for (source_doc_id, content) in &doc_content {
            let content_lower = content.to_lowercase();

            // Extract wikilinks: [[Title]]
            for cap in wikilink_re.captures_iter(content) {
                let ref_title = cap[1].trim().to_lowercase();
                if let Some(target_doc_id) = graph.title_index.get(&ref_title) {
                    if target_doc_id != source_doc_id {
                        edges_to_add.push((source_doc_id.clone(), target_doc_id.clone()));
                    }
                }
            }

            // Extract markdown relative links
            for cap in md_link_re.captures_iter(content) {
                let ref_name = cap[1].trim().to_lowercase();
                if let Some(target_doc_id) = graph.title_index.get(&ref_name) {
                    if target_doc_id != source_doc_id {
                        edges_to_add.push((source_doc_id.clone(), target_doc_id.clone()));
                    }
                }
            }

            // Extract title mentions: only for titles with len > 8 chars (avoids noise)
            for (title_variant, target_doc_id) in &graph.title_index {
                if title_variant.len() <= 8 {
                    continue;
                }
                if target_doc_id == source_doc_id {
                    continue;
                }
                if content_lower.contains(title_variant.as_str()) {
                    edges_to_add.push((source_doc_id.clone(), target_doc_id.clone()));
                }
            }
        }

        for (source, target) in edges_to_add {
            graph.add_edge(&source, &target);
        }

        tracing::info!(
            "[DOCUMENT_GRAPH] Built graph: {} documents, {} total edges",
            seen_docs.len(),
            graph.edges.values().map(|v| v.len()).sum::<usize>()
        );

        graph
    }

    fn add_edge(&mut self, source: &str, target: &str) {
        let targets = self.edges.entry(source.to_string()).or_default();
        if !targets.contains(&target.to_string()) {
            targets.push(target.to_string());
        }
    }

    /// Returns the direct graph neighbors of a single document.
    pub fn neighbors(&self, doc_id: &str) -> &[String] {
        self.edges.get(doc_id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Returns all unique neighbors of a set of documents, excluding the input docs themselves.
    /// Used for Hop-2 retrieval in recursive strategy.
    pub fn neighbors_of_set(&self, doc_ids: &[&str]) -> Vec<String> {
        let source_set: HashSet<&str> = doc_ids.iter().copied().collect();
        let mut result: HashSet<String> = HashSet::new();

        for doc_id in doc_ids {
            for neighbor in self.neighbors(doc_id) {
                if !source_set.contains(neighbor.as_str()) {
                    result.insert(neighbor.clone());
                }
            }
        }

        let mut out: Vec<String> = result.into_iter().collect();
        out.sort(); // stable order for deterministic behavior
        out
    }

    /// Returns true if the graph has been populated.
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    /// Returns the number of documents with outbound edges.
    pub fn node_count(&self) -> usize {
        self.edges.len()
    }

    /// Returns the total number of directed edges.
    pub fn edge_count(&self) -> usize {
        self.edges.values().map(|v| v.len()).sum()
    }

    /// Returns a flat list of all directed edges in the graph as (source, target, edge_type) tuples.
    pub fn all_edges(&self) -> Vec<(String, String, &'static str)> {
        let mut out = Vec::new();
        for (source, targets) in &self.edges {
            for target in targets {
                out.push((source.clone(), target.clone(), "link"));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ChunkSearchDocument;
    use serde_json::Value;

    fn make_doc(doc_id: &str, title: &str, content: &str) -> ChunkSearchDocument {
        ChunkSearchDocument {
            chunk_id: format!("{}-chunk-1", doc_id),
            document_id: doc_id.to_string(),
            ordinal: 0,
            source_kind: "obsidian".to_string(),
            title: title.to_string(),
            content: content.to_string(),
            path_or_url: None,
            tags: vec![],
            author: None,
            category: None,
            created_at: None,
            updated_at: None,
            metadata: Value::Object(Default::default()),
            chunk_metadata_json: None,
        }
    }

    #[test]
    fn test_document_graph_builds_edges_from_wikilinks() {
        let docs = vec![
            make_doc("doc-auth", "authentication_flow", "See also [[token management]] for details"),
            make_doc("doc-token", "token management", "Token storage and refresh logic"),
        ];
        let graph = DocumentGraph::build(&docs);
        let neighbors = graph.neighbors("doc-auth");
        assert!(
            neighbors.contains(&"doc-token".to_string()),
            "auth doc should link to token doc via wikilink"
        );
    }

    #[test]
    fn test_document_graph_neighbors_of_set() {
        let docs = vec![
            make_doc("doc-a", "document_alpha", "links to [[document_beta]] here"),
            make_doc("doc-b", "document_beta", "links to [[document_gamma]] here"),
            make_doc("doc-c", "document_gamma", "standalone document"),
        ];
        let graph = DocumentGraph::build(&docs);
        let hop2 = graph.neighbors_of_set(&["doc-a"]);
        assert!(hop2.contains(&"doc-b".to_string()));
    }

    #[test]
    fn test_no_self_loops() {
        let docs = vec![
            make_doc("doc-a", "self_referencing", "This doc is called self_referencing and refers to itself"),
        ];
        let graph = DocumentGraph::build(&docs);
        assert!(
            graph.neighbors("doc-a").is_empty(),
            "documents should not have self-loops"
        );
    }
}
