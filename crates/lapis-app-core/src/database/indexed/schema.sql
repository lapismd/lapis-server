CREATE TABLE IF NOT EXISTS files (
  path TEXT PRIMARY KEY,
  normalized_path TEXT NOT NULL,
  extension TEXT NOT NULL,
  mtime INTEGER NOT NULL,
  size INTEGER NOT NULL,
  hash TEXT NOT NULL,
  indexed INTEGER NOT NULL,
  deleted INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS files_manifest_idx
  ON files(deleted, indexed, path);
CREATE INDEX IF NOT EXISTS files_extension_path_idx
  ON files(extension, deleted, indexed, path);
CREATE TABLE IF NOT EXISTS metadata (
  path TEXT PRIMARY KEY,
  hash TEXT NOT NULL,
  parser_version TEXT NOT NULL,
  data_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS links (
  source_path TEXT NOT NULL,
  ordinal INTEGER NOT NULL,
  data_json TEXT NOT NULL,
  PRIMARY KEY (source_path, ordinal)
);
CREATE TABLE IF NOT EXISTS tags (
  path TEXT NOT NULL,
  ordinal INTEGER NOT NULL,
  data_json TEXT NOT NULL,
  PRIMARY KEY (path, ordinal)
);
CREATE TABLE IF NOT EXISTS properties (
  path TEXT NOT NULL,
  ordinal INTEGER NOT NULL,
  data_json TEXT NOT NULL,
  PRIMARY KEY (path, ordinal)
);
CREATE TABLE IF NOT EXISTS metadata_links (
  source_path TEXT NOT NULL,
  ordinal INTEGER NOT NULL,
  normalized_target TEXT NOT NULL,
  resolved_target_path TEXT,
  resolution_state TEXT NOT NULL,
  link_type TEXT NOT NULL,
  link_kind TEXT,
  data_json TEXT NOT NULL,
  PRIMARY KEY (source_path, ordinal)
);
CREATE INDEX IF NOT EXISTS metadata_links_incoming_idx
  ON metadata_links(resolved_target_path, source_path, ordinal);
CREATE INDEX IF NOT EXISTS metadata_links_unresolved_idx
  ON metadata_links(resolution_state, normalized_target, source_path);
CREATE TABLE IF NOT EXISTS metadata_tags (
  path TEXT NOT NULL,
  ordinal INTEGER NOT NULL,
  tag TEXT NOT NULL,
  normalized_tag TEXT NOT NULL,
  data_json TEXT NOT NULL,
  PRIMARY KEY (path, ordinal)
);
CREATE INDEX IF NOT EXISTS metadata_tags_exact_idx
  ON metadata_tags(normalized_tag, path);
CREATE TABLE IF NOT EXISTS metadata_tag_ancestors (
  path TEXT NOT NULL,
  tag_ordinal INTEGER NOT NULL,
  ancestor TEXT NOT NULL,
  depth INTEGER NOT NULL,
  PRIMARY KEY (path, tag_ordinal, ancestor)
);
CREATE INDEX IF NOT EXISTS metadata_tag_ancestors_idx
  ON metadata_tag_ancestors(ancestor, path);
CREATE TABLE IF NOT EXISTS metadata_properties (
  path TEXT NOT NULL,
  ordinal INTEGER NOT NULL,
  name TEXT NOT NULL,
  normalized_name TEXT NOT NULL,
  inferred_type TEXT NOT NULL,
  declared_type TEXT,
  data_json TEXT NOT NULL,
  PRIMARY KEY (path, ordinal)
);
CREATE INDEX IF NOT EXISTS metadata_properties_name_idx
  ON metadata_properties(normalized_name, path);
CREATE TABLE IF NOT EXISTS metadata_property_values (
  path TEXT NOT NULL,
  property_ordinal INTEGER NOT NULL,
  value_ordinal INTEGER NOT NULL,
  property_name TEXT NOT NULL,
  property_path TEXT NOT NULL,
  normalized_property_path TEXT NOT NULL,
  value_type TEXT NOT NULL,
  text_value TEXT,
  number_value REAL,
  boolean_value INTEGER,
  date_value TEXT,
  data_json TEXT NOT NULL,
  PRIMARY KEY (path, property_ordinal, value_ordinal, property_path)
);
CREATE INDEX IF NOT EXISTS metadata_property_text_idx
  ON metadata_property_values(property_name, text_value, path);
CREATE INDEX IF NOT EXISTS metadata_property_number_idx
  ON metadata_property_values(property_name, number_value, path);
CREATE INDEX IF NOT EXISTS metadata_property_boolean_idx
  ON metadata_property_values(property_name, boolean_value, path);
CREATE INDEX IF NOT EXISTS metadata_property_date_idx
  ON metadata_property_values(property_name, date_value, path);
