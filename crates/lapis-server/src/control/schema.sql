PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS installation_owner (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  username TEXT NOT NULL,
  password_hash TEXT NOT NULL,
  created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS sessions (
  id TEXT PRIMARY KEY,
  owner_id INTEGER NOT NULL REFERENCES installation_owner(id),
  token_hash TEXT NOT NULL UNIQUE,
  csrf_token TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  last_seen_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL,
  revoked_at INTEGER
);
CREATE INDEX IF NOT EXISTS sessions_token_idx ON sessions(token_hash);
CREATE TABLE IF NOT EXISTS login_attempts (
  key_hash TEXT PRIMARY KEY,
  window_started_at INTEGER NOT NULL,
  attempt_count INTEGER NOT NULL,
  blocked_until INTEGER
);
CREATE TABLE IF NOT EXISTS hosted_vaults (
  id TEXT PRIMARY KEY,
  owner_id INTEGER NOT NULL REFERENCES installation_owner(id) CHECK(owner_id = 1),
  name TEXT NOT NULL,
  managed INTEGER NOT NULL CHECK(managed IN (0,1)),
  physical_device TEXT NOT NULL,
  physical_inode TEXT NOT NULL,
  archived_at INTEGER
);
