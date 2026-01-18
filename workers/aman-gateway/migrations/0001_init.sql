CREATE TABLE IF NOT EXISTS nostr_events (
  event_id TEXT PRIMARY KEY,
  kind INTEGER NOT NULL,
  pubkey TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  d_tag TEXT,
  raw_json TEXT NOT NULL,
  seen_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS docs (
  doc_id TEXT PRIMARY KEY,
  title TEXT,
  lang TEXT,
  mime TEXT,
  updated_at INTEGER,
  manifest_event_id TEXT,
  content_hash TEXT,
  blob_ref TEXT
);

CREATE TABLE IF NOT EXISTS chunks (
  chunk_id TEXT PRIMARY KEY,
  doc_id TEXT NOT NULL,
  ord INTEGER,
  chunk_hash TEXT,
  blob_ref TEXT,
  text TEXT,
  created_at INTEGER,
  event_id TEXT
);

CREATE INDEX IF NOT EXISTS idx_chunks_doc_id ON chunks(doc_id);
CREATE INDEX IF NOT EXISTS idx_chunks_created_at ON chunks(created_at);

CREATE TABLE IF NOT EXISTS sync_state (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
  text,
  doc_id UNINDEXED,
  chunk_id UNINDEXED,
  title UNINDEXED
);
