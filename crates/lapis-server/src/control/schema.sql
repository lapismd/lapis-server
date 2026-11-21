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

CREATE TABLE IF NOT EXISTS content_access_settings (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  enabled INTEGER NOT NULL CHECK (enabled IN (0,1))
);

CREATE TABLE IF NOT EXISTS sync_account_connections (
  id TEXT PRIMARY KEY,
  client_id TEXT NOT NULL,
  client_name TEXT NOT NULL,
  s256_challenge TEXT NOT NULL,
  requested_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL,
  decision TEXT CHECK(decision IN ('approved','rejected')),
  decided_by_session TEXT REFERENCES sessions(id),
  decided_at INTEGER,
  consumed_at INTEGER,
  CHECK((decision IS NULL) = (decided_at IS NULL)),
  CHECK((decision IS NULL) = (decided_by_session IS NULL)),
  CHECK(consumed_at IS NULL OR decision = 'approved')
);
CREATE INDEX IF NOT EXISTS sync_account_connections_expiry_idx
  ON sync_account_connections(expires_at);

CREATE TABLE IF NOT EXISTS sync_account_credentials (
  id TEXT PRIMARY KEY,
  client_id TEXT NOT NULL,
  token_hash TEXT NOT NULL UNIQUE,
  issued_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL,
  revoked_at INTEGER
);
CREATE INDEX IF NOT EXISTS sync_account_credentials_token_idx
  ON sync_account_credentials(token_hash);

-- This association outlives the ten-minute consent request, so a native owner
-- can inspect/revoke an issued discovery credential after a lost response.
CREATE TABLE IF NOT EXISTS sync_account_outcomes (
  connection_id TEXT PRIMARY KEY,
  client_id TEXT NOT NULL,
  s256_challenge TEXT NOT NULL,
  connection_expires_at INTEGER NOT NULL,
  credential_id TEXT UNIQUE REFERENCES sync_account_credentials(id),
  cancelled_at INTEGER,
  CHECK(credential_id IS NOT NULL OR cancelled_at IS NOT NULL)
);

CREATE TABLE IF NOT EXISTS personal_access_tokens (
  id TEXT PRIMARY KEY,
  owner_id INTEGER NOT NULL REFERENCES installation_owner(id) CHECK(owner_id = 1),
  verifier TEXT NOT NULL UNIQUE,
  label TEXT NOT NULL,
  vault_ids TEXT NOT NULL,
  permissions TEXT NOT NULL,
  operations TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  expires_at INTEGER,
  last_used_at INTEGER,
  revoked_at INTEGER
);

CREATE TABLE IF NOT EXISTS owner_passkeys (
  id TEXT PRIMARY KEY,
  owner_id INTEGER NOT NULL REFERENCES installation_owner(id) CHECK(owner_id=1),
  credential_id TEXT NOT NULL UNIQUE,
  origin TEXT NOT NULL,
  label TEXT NOT NULL,
  credential TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  last_used_at INTEGER
);

CREATE TABLE IF NOT EXISTS session_reauthentication (
  session_id TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
  verified_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS mcp_oauth_clients (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  redirects TEXT NOT NULL,
  auth_method TEXT NOT NULL,
  secret_verifier TEXT,
  created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS mcp_oauth_grants (
  id TEXT PRIMARY KEY,
  owner_id INTEGER NOT NULL REFERENCES installation_owner(id) CHECK(owner_id=1),
  client_id TEXT NOT NULL,
  resource TEXT NOT NULL,
  label TEXT NOT NULL,
  vault_ids TEXT NOT NULL,
  permissions TEXT NOT NULL,
  operations TEXT NOT NULL,
  scope TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL,
  last_used_at INTEGER,
  revoked_at INTEGER,
  access_verifier TEXT NOT NULL UNIQUE,
  access_expires_at INTEGER NOT NULL,
  refresh_verifier TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS mcp_oauth_spent_refresh (
  verifier TEXT PRIMARY KEY,
  grant_id TEXT NOT NULL REFERENCES mcp_oauth_grants(id) ON DELETE CASCADE
);
