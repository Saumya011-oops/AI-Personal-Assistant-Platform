/// Topic Cluster Service — assigns documents to semantic topic clusters at startup.
///
/// Clustering is deterministic (no ML/LLM): for each document, we score it against
/// every entity group in the EntityDictionary by counting matched expansion terms in
/// the document's title + first 500 chars of content.
///
/// Documents are assigned to a cluster when:
///   1. Any primary_term appears verbatim in the document title (confidence = 1.0), OR
///   2. Score ≥ 0.15 (≥1-2 expansion terms match in content) (confidence = score)
///
/// Clusters are used for:
///   - Broad-topic retrieval: fetching all docs in a cluster for "Explain X" queries
///   - Ambiguity detection: docs in the same cluster are NEVER ambiguous
///   - Recursive retrieval: fallback cluster-based expansion when graph has no edges

use std::collections::HashMap;

use crate::domain::ChunkSearchDocument;
use crate::services::entity_dictionary::EntityDictionary;

#[derive(Debug, Clone, Default)]
pub struct TopicCluster {
    /// cluster_name → Vec<document_id>, ordered by cluster confidence desc
    cluster_to_docs: HashMap<String, Vec<(String, f32)>>, // (doc_id, confidence)
    /// document_id → Vec<cluster_name>
    doc_to_clusters: HashMap<String, Vec<String>>,
}

impl TopicCluster {
    /// Builds topic cluster assignments from a list of indexed documents.
    /// Rebuilds from scratch on each call (called at startup + after every sync).
    pub fn build(documents: &[ChunkSearchDocument], entity_dict: &EntityDictionary) -> Self {
        let mut cluster = TopicCluster::default();

        // Deduplicate: one entry per document (title + combined content snippet)
        let mut doc_snippets: HashMap<String, (String, String)> = HashMap::new(); // doc_id → (title, content_snippet)
        for doc in documents {
            doc_snippets
                .entry(doc.document_id.clone())
                .and_modify(|(_, content)| {
                    // Accumulate content up to 500 chars total
                    if content.len() < 500 {
                        content.push(' ');
                        content.push_str(&doc.content);
                        content.truncate(500);
                    }
                })
                .or_insert_with(|| {
                    let snippet: String = doc.content.chars().take(500).collect();
                    (doc.title.clone(), snippet)
                });
        }

        for (doc_id, (title, content_snippet)) in &doc_snippets {
            let title_lower = title.to_lowercase();
            let combined = format!("{} {}", title_lower, content_snippet.to_lowercase());

            let mut assigned_clusters: Vec<String> = Vec::new();

            for group in &entity_dict.groups {
                // Rule 1: any primary_term in title → confidence 1.0
                let title_match = group
                    .primary_terms
                    .iter()
                    .any(|t| title_lower.contains(t));

                if title_match {
                    let confidence = 1.0_f32;
                    cluster
                        .cluster_to_docs
                        .entry(group.name.to_string())
                        .or_default()
                        .push((doc_id.clone(), confidence));
                    assigned_clusters.push(group.name.to_string());
                    continue;
                }

                // Rule 2: score by expansion term matches
                let total_terms = group.expansion_terms.len() as f32;
                if total_terms == 0.0 {
                    continue;
                }
                let matches = group
                    .expansion_terms
                    .iter()
                    .filter(|t| combined.contains(t.as_str()))
                    .count() as f32;
                let score = matches / total_terms;

                if score >= 0.15 {
                    cluster
                        .cluster_to_docs
                        .entry(group.name.to_string())
                        .or_default()
                        .push((doc_id.clone(), score));
                    assigned_clusters.push(group.name.to_string());
                }
            }

            if !assigned_clusters.is_empty() {
                cluster
                    .doc_to_clusters
                    .insert(doc_id.clone(), assigned_clusters);
            }
        }

        // Sort each cluster by confidence descending
        for entries in cluster.cluster_to_docs.values_mut() {
            entries.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        }

        let total_assignments: usize = cluster.cluster_to_docs.values().map(|v| v.len()).sum();
        tracing::info!(
            "[TOPIC_CLUSTER] Built clusters: {} groups, {} documents, {} total assignments",
            cluster.cluster_to_docs.len(),
            doc_snippets.len(),
            total_assignments
        );
        for (cluster_name, docs) in &cluster.cluster_to_docs {
            tracing::debug!(
                "[TOPIC_CLUSTER] cluster={} docs={:?}",
                cluster_name,
                docs.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>()
            );
        }

        cluster
    }

    /// Returns all document IDs in the given cluster, ordered by confidence descending.
    pub fn docs_in_cluster(&self, cluster_name: &str) -> Vec<String> {
        self.cluster_to_docs
            .get(cluster_name)
            .map(|entries| entries.iter().map(|(id, _)| id.clone()).collect())
            .unwrap_or_default()
    }

    /// Returns all cluster names a document belongs to.
    pub fn clusters_for_doc(&self, doc_id: &str) -> &[String] {
        self.doc_to_clusters
            .get(doc_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Returns true if ALL provided document IDs belong to at least one common cluster.
    /// Used to replace Gate 2 in `is_ambiguous_retrieval`.
    pub fn same_cluster(&self, doc_ids: &[&str]) -> bool {
        if doc_ids.len() < 2 {
            return true; // trivially same cluster
        }
        // Get cluster sets for each doc
        let cluster_sets: Vec<std::collections::HashSet<&str>> = doc_ids
            .iter()
            .map(|id| {
                self.doc_to_clusters
                    .get(*id)
                    .map(|v| v.iter().map(|s| s.as_str()).collect())
                    .unwrap_or_default()
            })
            .collect();

        if cluster_sets.iter().any(|s| s.is_empty()) {
            return false;
        }

        // Check if the intersection of all cluster sets is non-empty
        let first = &cluster_sets[0];
        first
            .iter()
            .any(|cluster| cluster_sets.iter().all(|set| set.contains(cluster)))
    }

    /// Returns the cluster name shared by the most of the given document IDs, if any.
    pub fn dominant_cluster(&self, doc_ids: &[&str]) -> Option<String> {
        let mut cluster_counts: HashMap<&str, usize> = HashMap::new();
        for doc_id in doc_ids {
            for cluster in self.clusters_for_doc(doc_id) {
                *cluster_counts.entry(cluster.as_str()).or_default() += 1;
            }
        }
        cluster_counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .filter(|(_, count)| *count >= 2) // need at least 2 docs in same cluster
            .map(|(name, _)| name.to_string())
    }

    /// Returns true if the cluster has been populated.
    pub fn is_empty(&self) -> bool {
        self.cluster_to_docs.is_empty()
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
    fn test_topic_cluster_assigns_auth_docs_to_auth_cluster() {
        let docs = vec![
            make_doc("doc-1", "authentication_flow_oauth2", "oauth login token jwt session"),
            make_doc("doc-2", "authentication_sso_integration", "sso saml identity provider"),
            make_doc("doc-3", "monitoring_prometheus_grafana", "metrics alerts dashboard"),
        ];
        let dict = EntityDictionary::build(&docs);
        let cluster = TopicCluster::build(&docs, &dict);
        let auth_docs = cluster.docs_in_cluster("authentication");
        assert!(auth_docs.contains(&"doc-1".to_string()));
        assert!(auth_docs.contains(&"doc-2".to_string()));
        assert!(!auth_docs.contains(&"doc-3".to_string()), "monitoring doc should not be in auth cluster");
    }

    #[test]
    fn test_topic_cluster_same_cluster_true_for_auth_docs() {
        let docs = vec![
            make_doc("doc-1", "authentication_flow_oauth2", "oauth login token"),
            make_doc("doc-2", "authentication_sso_integration", "sso identity login"),
        ];
        let dict = EntityDictionary::build(&docs);
        let cluster = TopicCluster::build(&docs, &dict);
        assert!(cluster.same_cluster(&["doc-1", "doc-2"]));
    }

    #[test]
    fn test_topic_cluster_same_cluster_false_for_different_domains() {
        let docs = vec![
            make_doc("doc-1", "authentication_flow_oauth2", "oauth login token"),
            make_doc("doc-2", "monitoring_prometheus_grafana", "metrics alerts prometheus"),
        ];
        let dict = EntityDictionary::build(&docs);
        let cluster = TopicCluster::build(&docs, &dict);
        assert!(!cluster.same_cluster(&["doc-1", "doc-2"]));
    }

    #[test]
    fn test_dominant_cluster() {
        let docs = vec![
            make_doc("doc-1", "authentication_flow_oauth2", "oauth login token"),
            make_doc("doc-2", "authentication_sso_integration", "sso identity login"),
            make_doc("doc-3", "authentication_token_management", "token refresh expiry"),
        ];
        let dict = EntityDictionary::build(&docs);
        let cluster = TopicCluster::build(&docs, &dict);
        let dominant = cluster.dominant_cluster(&["doc-1", "doc-2", "doc-3"]);
        assert_eq!(dominant, Some("authentication".to_string()));
    }
}
