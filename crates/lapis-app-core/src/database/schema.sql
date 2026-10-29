CREATE TABLE IF NOT EXISTS schema_meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS app_meta (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS app_state (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  state_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS notifications (
  id TEXT PRIMARY KEY,
  data_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS history_files (
  file_id TEXT PRIMARY KEY,
  data_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS history_file_paths (
  path TEXT PRIMARY KEY,
  file_id TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS history_revisions (
  file_id TEXT NOT NULL,
  ordinal INTEGER NOT NULL,
  data_json TEXT NOT NULL,
  PRIMARY KEY (file_id, ordinal)
);
CREATE TABLE IF NOT EXISTS app_revision (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  revision INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS app_changes (
  revision INTEGER PRIMARY KEY,
  domains_json TEXT NOT NULL,
  paths_json TEXT NOT NULL,
  renamed_json TEXT,
  committed_at INTEGER NOT NULL
);
INSERT INTO app_revision(id, revision) VALUES(1, 0) ON CONFLICT(id) DO NOTHING;
