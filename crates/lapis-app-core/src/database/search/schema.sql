CREATE TABLE IF NOT EXISTS search_docs (
  path TEXT PRIMARY KEY, source_provider_id TEXT, metadata_hash TEXT,
  provider_version TEXT, projection_signature TEXT,
  source_mtime INTEGER, source_size INTEGER,
  name TEXT NOT NULL, extension TEXT NOT NULL, checksum TEXT NOT NULL,
  content TEXT NOT NULL, tags TEXT NOT NULL, metadata_text TEXT NOT NULL,
  data_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS search_chunks (
  id TEXT PRIMARY KEY, path TEXT NOT NULL, text TEXT NOT NULL,
  embedding_json TEXT, embedding BLOB
);
CREATE INDEX IF NOT EXISTS search_chunks_path_idx ON search_chunks(path);
