CREATE TABLE IF NOT EXISTS memory_source_state (
  source_key TEXT PRIMARY KEY, source_path TEXT NOT NULL,
  source_hash TEXT NOT NULL, status TEXT NOT NULL,
  indexed_at INTEGER NOT NULL, data_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS memory_candidates (
  id TEXT PRIMARY KEY, scope_kind TEXT NOT NULL, scope_path TEXT NOT NULL,
  state TEXT NOT NULL, last_seen_at INTEGER NOT NULL, data_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS memory_candidates_scope_state
  ON memory_candidates(scope_kind, scope_path, state, last_seen_at DESC);
CREATE TABLE IF NOT EXISTS memory_candidate_origins (
  candidate_id TEXT NOT NULL, conversation_id TEXT NOT NULL,
  entry_id TEXT NOT NULL, data_json TEXT NOT NULL,
  PRIMARY KEY (candidate_id, conversation_id, entry_id)
);
CREATE INDEX IF NOT EXISTS memory_candidate_origins_conversation
  ON memory_candidate_origins(conversation_id, candidate_id);
CREATE TABLE IF NOT EXISTS memory_recall_signals (
  target_ref TEXT NOT NULL, query_fingerprint TEXT NOT NULL, day TEXT NOT NULL,
  best_score REAL NOT NULL, hit_count INTEGER NOT NULL, data_json TEXT NOT NULL,
  PRIMARY KEY (target_ref, query_fingerprint, day)
);
CREATE TABLE IF NOT EXISTS memory_jobs (
  id TEXT PRIMARY KEY, scope_key TEXT NOT NULL, status TEXT NOT NULL,
  lease_until INTEGER, created_at INTEGER NOT NULL, data_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS memory_jobs_scope_created
  ON memory_jobs(scope_key, created_at DESC);
