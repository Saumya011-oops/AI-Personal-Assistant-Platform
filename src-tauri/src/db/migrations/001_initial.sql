CREATE TABLE IF NOT EXISTS integrations (
  key TEXT PRIMARY KEY,
  label TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'not_connected',
  detail TEXT,
  last_synced_at TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS settings (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  obsidian_vault_path TEXT,
  preferred_theme TEXT NOT NULL DEFAULT 'dark',
  command_palette_enabled INTEGER NOT NULL DEFAULT 1,
  telemetry_enabled INTEGER NOT NULL DEFAULT 1,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT OR IGNORE INTO settings (id) VALUES (1);

CREATE TABLE IF NOT EXISTS documents (
  id TEXT PRIMARY KEY,
  source_kind TEXT NOT NULL,
  source_external_id TEXT NOT NULL,
  title TEXT NOT NULL,
  content_markdown TEXT NOT NULL,
  content_plaintext TEXT NOT NULL,
  path_or_url TEXT,
  tags_json TEXT NOT NULL DEFAULT '[]',
  checksum TEXT NOT NULL,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT,
  updated_at TEXT,
  ingested_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  UNIQUE(source_kind, source_external_id)
);

CREATE INDEX IF NOT EXISTS idx_documents_source_kind ON documents(source_kind);
CREATE INDEX IF NOT EXISTS idx_documents_checksum ON documents(checksum);
CREATE INDEX IF NOT EXISTS idx_documents_updated_at ON documents(updated_at);

CREATE TABLE IF NOT EXISTS chunks (
  id TEXT PRIMARY KEY,
  document_id TEXT NOT NULL,
  ordinal INTEGER NOT NULL,
  content TEXT NOT NULL,
  token_count INTEGER NOT NULL DEFAULT 0,
  embedding_status TEXT NOT NULL DEFAULT 'pending',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY(document_id) REFERENCES documents(id) ON DELETE CASCADE,
  UNIQUE(document_id, ordinal)
);

CREATE INDEX IF NOT EXISTS idx_chunks_document_id ON chunks(document_id);
CREATE INDEX IF NOT EXISTS idx_chunks_embedding_status ON chunks(embedding_status);

CREATE VIRTUAL TABLE IF NOT EXISTS chunk_fts USING fts5(
  chunk_id UNINDEXED,
  document_id UNINDEXED,
  source,
  title,
  content,
  tags,
  author,
  category,
  created_at UNINDEXED,
  updated_at UNINDEXED,
  tokenize = 'porter unicode61'
);

CREATE TABLE IF NOT EXISTS sync_state (
  id TEXT PRIMARY KEY,
  integration_key TEXT NOT NULL,
  status TEXT NOT NULL,
  started_at TEXT NOT NULL,
  finished_at TEXT,
  documents_discovered INTEGER NOT NULL DEFAULT 0,
  documents_upserted INTEGER NOT NULL DEFAULT 0,
  error_message TEXT,
  FOREIGN KEY(integration_key) REFERENCES integrations(key)
);

CREATE INDEX IF NOT EXISTS idx_sync_state_integration_key ON sync_state(integration_key);
CREATE INDEX IF NOT EXISTS idx_sync_state_started_at ON sync_state(started_at);

CREATE TABLE IF NOT EXISTS credentials (
  provider TEXT PRIMARY KEY,
  account_identifier TEXT NOT NULL,
  encrypted_token_blob TEXT NOT NULL,
  scopes_json TEXT NOT NULL DEFAULT '[]',
  expires_at TEXT,
  last_refresh_at TEXT,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS chat_messages (
  id TEXT PRIMARY KEY,
  conversation_id TEXT NOT NULL,
  role TEXT NOT NULL,
  content TEXT NOT NULL,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_chat_messages_conversation_id ON chat_messages(conversation_id, created_at);
