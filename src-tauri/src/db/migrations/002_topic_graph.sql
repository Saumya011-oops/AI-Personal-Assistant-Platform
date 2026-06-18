-- Phase 2: Topic Graph Tables
-- document_clusters: maps each document to one or more topic clusters (e.g. "authentication", "monitoring")
CREATE TABLE IF NOT EXISTS document_clusters (
  document_id TEXT NOT NULL,
  cluster_id  TEXT NOT NULL,   -- entity group name, e.g. "authentication"
  confidence  REAL NOT NULL DEFAULT 1.0,
  PRIMARY KEY (document_id, cluster_id),
  FOREIGN KEY(document_id) REFERENCES documents(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_document_clusters_cluster ON document_clusters(cluster_id);
CREATE INDEX IF NOT EXISTS idx_document_clusters_doc    ON document_clusters(document_id);

-- document_graph_edges: directional links extracted at indexing time
-- edge_type: 'wikilink' | 'md_link' | 'title_mention'
CREATE TABLE IF NOT EXISTS document_graph_edges (
  source_doc_id TEXT NOT NULL,
  target_doc_id TEXT NOT NULL,
  edge_type     TEXT NOT NULL,
  PRIMARY KEY (source_doc_id, target_doc_id, edge_type),
  FOREIGN KEY(source_doc_id) REFERENCES documents(id) ON DELETE CASCADE,
  FOREIGN KEY(target_doc_id) REFERENCES documents(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_graph_edges_source ON document_graph_edges(source_doc_id);
CREATE INDEX IF NOT EXISTS idx_graph_edges_target ON document_graph_edges(target_doc_id);
