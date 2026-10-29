CREATE TABLE IF NOT EXISTS index_projections (
  projection_id TEXT PRIMARY KEY,
  owner_plugin_id TEXT NOT NULL,
  schema_version INTEGER NOT NULL,
  config_hash TEXT NOT NULL,
  visibility TEXT NOT NULL,
  fields_json TEXT NOT NULL,
  active INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS index_projection_sources (
  projection_id TEXT NOT NULL,
  source_path TEXT NOT NULL,
  source_hash TEXT NOT NULL,
  schema_version INTEGER NOT NULL,
  config_hash TEXT NOT NULL,
  status TEXT NOT NULL,
  error TEXT,
  indexed_at INTEGER NOT NULL,
  PRIMARY KEY (projection_id, source_path)
);
CREATE TABLE IF NOT EXISTS index_projection_rows (
  projection_id TEXT NOT NULL,
  row_id TEXT NOT NULL,
  source_path TEXT NOT NULL,
  kind TEXT NOT NULL,
  ordinal INTEGER NOT NULL DEFAULT 0,
  data_json TEXT NOT NULL,
  PRIMARY KEY (projection_id, row_id)
);
CREATE INDEX IF NOT EXISTS index_projection_rows_source
  ON index_projection_rows(projection_id, source_path);
CREATE TABLE IF NOT EXISTS index_projection_values (
  projection_id TEXT NOT NULL,
  row_id TEXT NOT NULL,
  field TEXT NOT NULL,
  ordinal INTEGER NOT NULL DEFAULT 0,
  value_type TEXT NOT NULL,
  text_value TEXT,
  number_value REAL,
  boolean_value INTEGER,
  date_value TEXT,
  datetime_value INTEGER,
  PRIMARY KEY (projection_id, row_id, field, ordinal)
);
CREATE TABLE IF NOT EXISTS index_projection_edges (
  projection_id TEXT NOT NULL,
  source_row_id TEXT NOT NULL,
  relation TEXT NOT NULL,
  target_projection_id TEXT,
  target_row_id TEXT,
  target_path TEXT,
  target_text TEXT,
  ordinal INTEGER NOT NULL,
  data_json TEXT,
  PRIMARY KEY (projection_id, source_row_id, relation, ordinal)
);
