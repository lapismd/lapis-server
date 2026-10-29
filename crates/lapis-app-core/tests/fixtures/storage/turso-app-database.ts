import { rebindSearchDocument } from "./search-embedding-record";
import { collectSearchQueryTerms } from "../search-query";
import {
  buildSearchResult,
  compareSearchResults,
  embeddingErrorState,
  EMPTY_APP_DATABASE_CAPABILITIES,
  hasSearchPropertyNames,
  MIN_VECTOR_SEARCH_SCORE,
  normalizeSearchDocument,
  pathWithinPrefix,
  rankSearchScores,
  resolveSearchQueryEnhancementDiagnostics,
  scoreSearchDocument,
  scoreVectorDocument,
  searchDocumentProperties,
  searchPropertyNames,
  type AppDatabase,
  type AppDatabaseChangeDomain,
  type AppDatabaseChangeListener,
  type AppDatabaseChangeSet,
  type AppDatabaseDescriptor,
  type AppDatabaseFileHistory,
  type AppDatabaseFileHistoryFile,
  type AppDatabaseFileHistoryRevision,
  type AppDatabaseIndexedFile,
  type AppDatabaseIndexedFileManifestPage,
  type AppDatabaseIndexedFileManifestQuery,
  type AppDatabaseIndexedMetadataPage,
  type AppDatabaseIndexedMetadataPageQuery,
  type AppDatabaseIndexedMetadataDomain,
  type AppDatabaseIndexedMetadataPropertyFilter,
  type AppDatabaseIndexedMetadataQuery,
  type AppDatabaseIndexedMetadataRow,
  type AppDatabaseKind,
  type AppDatabaseLinkRecord,
  type AppDatabaseMetadataFacetQuery,
  type AppDatabaseMetadataFacetRow,
  type AppDatabaseMetadataLinkQuery,
  type AppDatabaseMemoryCandidateInput,
  type AppDatabaseMemoryCandidateOriginRecord,
  type AppDatabaseMemoryCandidateQuery,
  type AppDatabaseMemoryCandidateRecord,
  type AppDatabaseMemoryCandidateResult,
  type AppDatabaseMemoryJobClaimInput,
  type AppDatabaseMemoryJobRecord,
  type AppDatabaseMemoryJobUpdateInput,
  type AppDatabaseMemoryRecallSignalRecord,
  type AppDatabaseMemorySourceRecord,
  type AppDatabaseMetaScanPage,
  type AppDatabaseMetaScanQuery,
  type AppDatabaseMetaTransaction,
  type AppDatabaseMetaTransactionResult,
  type AppDatabaseNotebookState,
  type AppDatabaseNotificationRecord,
  type AppDatabaseOpenContext,
  type AppDatabaseProvider,
  type AppDatabaseSearchIndexStats,
  type AppDatabaseSearchOptions,
  type AppDatabaseSearchResult,
  type AppDatabaseStoreFileHistoryRevisionInput,
  type AppDatabaseStoreFileHistoryRevisionResult,
  type MetadataCacheSnapshot,
  type SearchDocumentRecord,
  type SearchDocumentManifestPage,
  type SearchDocumentManifestQuery,
  type SearchEmbeddingProviderConfig,
} from "./app-database";
import {
  assertProjectionReadAccess,
  assertProjectionWriteAccess,
  compileProjectionQuerySql,
  evaluateProjectionQuery,
  indexedValuesForRow,
  MAX_PROJECTION_ROWS_PER_SOURCE,
  projectionIndexStatus,
  PUBLIC_TASKS_PROJECTION_ID,
  type IndexProjectionDefinitionRecord,
  type IndexProjectionEdgeRecord,
  type IndexProjectionRowRecord,
  type IndexProjectionSourceRecord,
  type IndexQuery,
  type IndexQueryResult,
  type IndexRelatedQuery,
  type MarkProjectionSourceErrorInput,
  type ReplaceProjectionSourceInput,
} from "./index-projection";
import { projectionQueryCandidates } from "./projection-query-candidates";
import type {
  AppDatabaseTaskChildQuery,
  AppDatabaseTaskQuery,
  AppDatabaseTaskRecord,
} from "./task-projection";
import {
  TASK_PROJECTION_FIELDS,
  TASK_PROJECTION_VERSION,
  taskQueryToIndexQuery,
} from "./task-projection";
import {
  createSearchEmbeddingProvider,
  type SearchEmbeddingProvider,
  type SearchEmbeddingRuntimeStatus,
} from "./search-embedding-provider";
import { isStructuredSearchQuery } from "./search-query-evaluator";

export interface TursoStatementInput {
  sql: string;
  args?: unknown[] | Record<string, unknown>;
}

export interface TursoConnection {
  exec(sql: string): Promise<void>;
  run(sql: string, ...args: unknown[]): Promise<unknown>;
  get<T extends Record<string, unknown>>(
    sql: string,
    ...args: unknown[]
  ): Promise<T | undefined>;
  all<T extends Record<string, unknown>>(
    sql: string,
    ...args: unknown[]
  ): Promise<T[]>;
  batch(
    statements: Array<string | TursoStatementInput>,
    options?: string,
  ): Promise<unknown>;
  close(): Promise<void>;
}

export type TursoConnectionFactory = () => Promise<TursoConnection>;

export interface TursoAppDatabaseOptions {
  kind: Extract<AppDatabaseKind, "turso-native" | "turso-wasm">;
  providerId?: string;
  transport: AppDatabaseDescriptor["transport"];
  role?: AppDatabaseDescriptor["role"];
  crossTabCoordination?: boolean;
  connectionFactory: TursoConnectionFactory;
}

export const TURSO_APP_DATABASE_SCHEMA_VERSION = 5;

export const TURSO_APP_DATABASE_SCHEMA = `
CREATE TABLE IF NOT EXISTS schema_meta (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS app_state (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  state_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS app_meta (
  key TEXT PRIMARY KEY,
  value_json TEXT NOT NULL
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
CREATE TABLE IF NOT EXISTS tasks (
  document_path TEXT PRIMARY KEY,
  document_id TEXT NOT NULL,
  kind TEXT NOT NULL,
  title TEXT NOT NULL,
  status TEXT NOT NULL,
  inbox INTEGER NOT NULL DEFAULT 0,
  start_kind TEXT NOT NULL,
  start_date TEXT,
  plan_date TEXT,
  plan_kind TEXT,
  plan_time TEXT,
  duration_minutes INTEGER,
  deadline TEXT,
  completed_at TEXT,
  repeat_strategy TEXT,
  repeat_frequency TEXT,
  repeat_interval INTEGER,
  repeat_anchor TEXT,
  checklist_total INTEGER NOT NULL DEFAULT 0,
  checklist_completed INTEGER NOT NULL DEFAULT 0,
  comment_count INTEGER NOT NULL DEFAULT 0,
  projection_version INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS task_records (
  document_path TEXT PRIMARY KEY,
  data_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS tasks_open_plan ON tasks(status, plan_date);
CREATE INDEX IF NOT EXISTS tasks_open_deadline ON tasks(status, deadline);
CREATE INDEX IF NOT EXISTS tasks_open_start ON tasks(status, start_kind, start_date);
CREATE INDEX IF NOT EXISTS tasks_inbox ON tasks(status, inbox);
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
CREATE TABLE IF NOT EXISTS notifications (
  id TEXT PRIMARY KEY,
  data_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS search_docs (
  path TEXT PRIMARY KEY,
  source_provider_id TEXT,
  metadata_hash TEXT,
  provider_version TEXT,
  projection_signature TEXT,
  source_mtime INTEGER,
  source_size INTEGER,
  name TEXT NOT NULL,
  extension TEXT NOT NULL,
  checksum TEXT NOT NULL,
  content TEXT NOT NULL,
  tags TEXT NOT NULL,
  metadata_text TEXT NOT NULL,
  data_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS search_chunks (
  id TEXT PRIMARY KEY,
  path TEXT NOT NULL,
  text TEXT NOT NULL,
  embedding_json TEXT,
  embedding BLOB
);
CREATE INDEX IF NOT EXISTS search_chunks_path_idx ON search_chunks(path);
CREATE TABLE IF NOT EXISTS memory_source_state (
  source_key TEXT PRIMARY KEY,
  source_path TEXT NOT NULL,
  source_hash TEXT NOT NULL,
  status TEXT NOT NULL,
  indexed_at INTEGER NOT NULL,
  data_json TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS memory_candidates (
  id TEXT PRIMARY KEY,
  scope_kind TEXT NOT NULL,
  scope_path TEXT NOT NULL,
  state TEXT NOT NULL,
  last_seen_at INTEGER NOT NULL,
  data_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS memory_candidates_scope_state
  ON memory_candidates(scope_kind, scope_path, state, last_seen_at DESC);
CREATE TABLE IF NOT EXISTS memory_candidate_origins (
  candidate_id TEXT NOT NULL,
  conversation_id TEXT NOT NULL,
  entry_id TEXT NOT NULL,
  data_json TEXT NOT NULL,
  PRIMARY KEY (candidate_id, conversation_id, entry_id)
);
CREATE INDEX IF NOT EXISTS memory_candidate_origins_conversation
  ON memory_candidate_origins(conversation_id, candidate_id);
CREATE TABLE IF NOT EXISTS memory_recall_signals (
  target_ref TEXT NOT NULL,
  query_fingerprint TEXT NOT NULL,
  day TEXT NOT NULL,
  best_score REAL NOT NULL,
  hit_count INTEGER NOT NULL,
  data_json TEXT NOT NULL,
  PRIMARY KEY (target_ref, query_fingerprint, day)
);
CREATE TABLE IF NOT EXISTS memory_jobs (
  id TEXT PRIMARY KEY,
  scope_key TEXT NOT NULL,
  status TEXT NOT NULL,
  lease_until INTEGER,
  created_at INTEGER NOT NULL,
  data_json TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS memory_jobs_scope_created
  ON memory_jobs(scope_key, created_at DESC);
`;

const TURSO_SEARCH_FTS_SCHEMA = `
CREATE INDEX IF NOT EXISTS search_docs_fts ON search_docs USING fts (
  name,
  path,
  content,
  tags,
  metadata_text
) WITH (weights = 'name=2.0,path=1.5,content=1.0,tags=1.5,metadata_text=1.0');
`;

function clone<T>(value: T): T {
  if (value === undefined || value === null) return value;
  return structuredClone(value);
}

function stableVaultHash(value: string): string {
  let hash = 2166136261;
  for (const character of value) {
    hash ^= character.codePointAt(0) ?? 0;
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0).toString(16).padStart(8, "0");
}

export function tursoWasmDatabasePath(vaultId: string): string {
  return `lapis-app-${stableVaultHash(vaultId)}.turso`;
}

function ftsQueryForTerms(terms: string[]): string {
  return terms
    .map((term) => `"${term.replaceAll("\\", "\\\\").replaceAll('"', '\\"')}"`)
    .join(" AND ");
}

const METADATA_SNAPSHOT_META_KEY = "compat.metadataSnapshot";
const SEARCH_EMBEDDING_PROVIDER_META_KEY = "search.embeddingProvider";
const MAX_BOUND_PATHS_PER_QUERY = 400;

function chunkPaths(paths: string[]): string[][] {
  const chunks: string[][] = [];
  for (
    let index = 0;
    index < paths.length;
    index += MAX_BOUND_PATHS_PER_QUERY
  ) {
    chunks.push(paths.slice(index, index + MAX_BOUND_PATHS_PER_QUERY));
  }
  return chunks;
}

function balancedSqlDisjunction(clauses: string[]): string {
  if (!clauses.length) return "0";
  let level = [...clauses];
  while (level.length > 1) {
    const next: string[] = [];
    for (let index = 0; index < level.length; index += 2) {
      const left = level[index]!;
      const right = level[index + 1];
      next.push(right ? `(${left} OR ${right})` : left);
    }
    level = next;
  }
  return level[0]!;
}

function parseJson<T>(value: unknown, fallback: T): T {
  if (typeof value !== "string") return fallback;
  try {
    return JSON.parse(value) as T;
  } catch {
    return fallback;
  }
}

function notebookStateMetaKey(sourcePath: string): string {
  return `notebook.state:${sourcePath}`;
}

function createStableId(prefix: string): string {
  if (
    typeof crypto !== "undefined" &&
    typeof crypto.randomUUID === "function"
  ) {
    return `${prefix}:${crypto.randomUUID()}`;
  }
  return `${prefix}:${Date.now().toString(36)}:${Math.random().toString(36).slice(2)}`;
}

function hashText(value: string): string {
  let hash = 2166136261;
  for (const character of value) {
    hash ^= character.codePointAt(0) ?? 0;
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0).toString(16).padStart(8, "0");
}

function normalizeTag(value: string): string {
  return value.trim().replace(/^#/, "").toLowerCase();
}

function normalizeExtension(value: string): string {
  return value.trim().replace(/^\./, "").toLowerCase();
}

function isDateScalar(value: string): boolean {
  return /^\d{4}-\d{2}-\d{2}(?:$|T)/.test(value);
}

interface FlattenedPropertyValue {
  path: string;
  value: unknown;
}

function flattenPropertyValues(
  value: unknown,
  path: string,
  output: FlattenedPropertyValue[] = [],
): FlattenedPropertyValue[] {
  if (Array.isArray(value)) {
    value.forEach((entry, index) =>
      flattenPropertyValues(entry, `${path}[${index}]`, output),
    );
    return output;
  }
  if (value && typeof value === "object") {
    for (const [key, entry] of Object.entries(
      value as Record<string, unknown>,
    )) {
      flattenPropertyValues(entry, path ? `${path}.${key}` : key, output);
    }
    return output;
  }
  output.push({ path, value });
  return output;
}

function normalizePropertyPath(path: string): string {
  return path.toLowerCase().replace(/\[\d+\]/gu, "[]");
}

function indexedFileStatements(
  record: AppDatabaseIndexedFile,
): TursoStatementInput[] {
  const path = record.file.path;
  const statements: TursoStatementInput[] = [
    {
      sql: `INSERT INTO files
            (path, normalized_path, extension, mtime, size, hash, indexed, deleted)
            VALUES (?, ?, ?, ?, ?, ?, ?, 0)
            ON CONFLICT(path) DO UPDATE SET
              normalized_path = excluded.normalized_path,
              extension = excluded.extension,
              mtime = excluded.mtime,
              size = excluded.size,
              hash = excluded.hash,
              indexed = excluded.indexed,
              deleted = 0`,
      args: [
        path,
        record.file.normalizedPath,
        normalizeExtension(record.file.extension),
        record.file.mtime,
        record.file.size,
        record.file.hash,
        record.file.indexed ? 1 : 0,
      ],
    },
    {
      sql: `INSERT INTO metadata (path, hash, parser_version, data_json)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(path) DO UPDATE SET
              hash = excluded.hash,
              parser_version = excluded.parser_version,
              data_json = excluded.data_json`,
      args: [
        record.metadata.path,
        record.metadata.hash,
        record.metadata.parserVersion,
        JSON.stringify(record.metadata.metadata),
      ],
    },
    { sql: "DELETE FROM links WHERE source_path = ?", args: [path] },
    { sql: "DELETE FROM tags WHERE path = ?", args: [path] },
    { sql: "DELETE FROM properties WHERE path = ?", args: [path] },
    { sql: "DELETE FROM metadata_links WHERE source_path = ?", args: [path] },
    { sql: "DELETE FROM metadata_tags WHERE path = ?", args: [path] },
    { sql: "DELETE FROM metadata_tag_ancestors WHERE path = ?", args: [path] },
    { sql: "DELETE FROM metadata_properties WHERE path = ?", args: [path] },
    {
      sql: "DELETE FROM metadata_property_values WHERE path = ?",
      args: [path],
    },
  ];

  record.links.forEach((link, ordinal) => {
    const data = { ...link, ordinal: link.ordinal ?? ordinal };
    statements.push(
      {
        sql: "INSERT INTO links (source_path, ordinal, data_json) VALUES (?, ?, ?)",
        args: [path, ordinal, JSON.stringify(data)],
      },
      {
        sql: `INSERT INTO metadata_links
              (source_path, ordinal, normalized_target, resolved_target_path, resolution_state, link_type, link_kind, data_json)
              VALUES (?, ?, ?, ?, ?, ?, ?, ?)`,
        args: [
          path,
          ordinal,
          link.targetText.trim().toLowerCase(),
          link.resolvedTargetPath,
          link.resolvedTargetPath ? "resolved" : "unresolved",
          link.type,
          link.kind ?? null,
          JSON.stringify(data),
        ],
      },
    );
  });

  record.tags.forEach((tag, ordinal) => {
    statements.push(
      {
        sql: "INSERT INTO tags (path, ordinal, data_json) VALUES (?, ?, ?)",
        args: [path, ordinal, JSON.stringify(tag)],
      },
      {
        sql: `INSERT INTO metadata_tags
              (path, ordinal, tag, normalized_tag, data_json)
              VALUES (?, ?, ?, ?, ?)`,
        args: [
          path,
          ordinal,
          tag.tag,
          normalizeTag(tag.tag),
          JSON.stringify(tag),
        ],
      },
    );
    [...new Set(tag.hierarchy.map(normalizeTag))].forEach((ancestor, depth) => {
      statements.push({
        sql: `INSERT INTO metadata_tag_ancestors
              (path, tag_ordinal, ancestor, depth) VALUES (?, ?, ?, ?)`,
        args: [path, ordinal, ancestor, depth],
      });
    });
  });

  record.properties.forEach((property, ordinal) => {
    const normalizedName = property.name.trim().toLowerCase();
    statements.push(
      {
        sql: "INSERT INTO properties (path, ordinal, data_json) VALUES (?, ?, ?)",
        args: [path, ordinal, JSON.stringify(property)],
      },
      {
        sql: `INSERT INTO metadata_properties
              (path, ordinal, name, normalized_name, inferred_type, declared_type, data_json)
              VALUES (?, ?, ?, ?, ?, ?, ?)`,
        args: [
          path,
          ordinal,
          property.name,
          normalizedName,
          property.inferredType,
          property.declaredType ?? null,
          JSON.stringify(property),
        ],
      },
    );
    flattenPropertyValues(property.value, property.name).forEach(
      (entry, valueOrdinal) => {
        const value = entry.value;
        const valueType =
          value === null || value === undefined
            ? "null"
            : typeof value === "number"
              ? "number"
              : typeof value === "boolean"
                ? "boolean"
                : typeof value === "string" && isDateScalar(value)
                  ? "date"
                  : "string";
        statements.push({
          sql: `INSERT INTO metadata_property_values
              (path, property_ordinal, value_ordinal, property_name, property_path,
               normalized_property_path,
               value_type, text_value, number_value, boolean_value, date_value, data_json)
              VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
          args: [
            path,
            ordinal,
            valueOrdinal,
            normalizedName,
            entry.path,
            normalizePropertyPath(entry.path),
            valueType,
            valueType === "string" ? String(value) : null,
            valueType === "number" ? value : null,
            valueType === "boolean" ? (value ? 1 : 0) : null,
            valueType === "date" ? String(value) : null,
            JSON.stringify(value ?? null),
          ],
        });
      },
    );
  });

  return statements;
}

export class TursoAppDatabase implements AppDatabase {
  readonly kind: Extract<AppDatabaseKind, "turso-native" | "turso-wasm">;

  private connection: TursoConnection | null = null;
  private opened = false;
  private closeTask: Promise<void> | null = null;
  private nativeFullTextSearch = false;
  private nativeVectorSearch = false;
  private searchEmbeddingProviderConfig: SearchEmbeddingProviderConfig | null =
    null;
  private searchEmbeddingProvider: SearchEmbeddingProvider | null = null;
  private currentRevision = 0;
  private commitQueue: Promise<void> = Promise.resolve();
  private readonly changeListeners = new Set<AppDatabaseChangeListener>();

  constructor(
    readonly vaultId: string,
    private readonly options: TursoAppDatabaseOptions,
  ) {
    this.kind = options.kind;
  }

  get descriptor(): AppDatabaseDescriptor {
    return {
      providerId: this.options.providerId ?? "turso-local",
      engine: "turso",
      transport: this.options.transport,
      role: this.options.role ?? "direct",
      storageMode: "local",
      capabilities: {
        ...EMPTY_APP_DATABASE_CAPABILITIES,
        nativeFullTextSearch: this.nativeFullTextSearch,
        vectorSearch: this.nativeVectorSearch,
        localEmbeddings: true,
        crossTabCoordination: this.options.crossTabCoordination ?? false,
      },
    };
  }

  async open(): Promise<void> {
    if (this.opened) return;
    this.connection = await this.options.connectionFactory();
    await this.migrate();
    const revision = await this.connection.get<{ revision: number }>(
      "SELECT revision FROM app_revision WHERE id = 1",
    );
    this.currentRevision = Number(revision?.revision ?? 0);
    this.searchEmbeddingProviderConfig =
      (await this.getMeta<SearchEmbeddingProviderConfig>(
        SEARCH_EMBEDDING_PROVIDER_META_KEY,
      )) ?? null;
    this.searchEmbeddingProvider = createSearchEmbeddingProvider(
      this.searchEmbeddingProviderConfig,
    );
    this.opened = true;
  }

  async migrate(): Promise<void> {
    const connection = this.requireConnection();
    await connection.exec(TURSO_APP_DATABASE_SCHEMA);
    await this.ensureMetadataPropertyPathSchema();
    const previousVersion = Number(
      (
        await connection.get<{ value: string }>(
          "SELECT value FROM schema_meta WHERE key = 'schema.version'",
        )
      )?.value ?? 0,
    );
    if (
      previousVersion > 0 &&
      previousVersion < TURSO_APP_DATABASE_SCHEMA_VERSION
    ) {
      const invariants = await this.captureMigrationInvariants();
      await this.backfillNormalizedMetadata();
      await connection.run(`UPDATE search_docs SET
        source_provider_id = json_extract(data_json, '$.sourceProviderId'),
        metadata_hash = json_extract(data_json, '$.sourceMetadata.metadataHash'),
        provider_version = json_extract(data_json, '$.sourceMetadata.providerVersion'),
        projection_signature = json_extract(data_json, '$.sourceMetadata.projectionSignature'),
        source_mtime = json_extract(data_json, '$.sourceMetadata.sourceMtime'),
        source_size = json_extract(data_json, '$.sourceMetadata.sourceSize')`);
      await this.validateMigration(invariants);
    }
    await connection.batch(
      [
        `INSERT INTO app_revision (id, revision) VALUES (1, 0)
         ON CONFLICT(id) DO NOTHING`,
        {
          sql: `INSERT INTO schema_meta (key, value) VALUES ('schema.version', ?)
                ON CONFLICT(key) DO UPDATE SET value = excluded.value`,
          args: [String(TURSO_APP_DATABASE_SCHEMA_VERSION)],
        },
        {
          sql: `INSERT INTO app_meta (key, value_json) VALUES ('schema.version', ?)
                ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json`,
          args: [JSON.stringify(TURSO_APP_DATABASE_SCHEMA_VERSION)],
        },
      ],
      "immediate",
    );
    try {
      await connection.exec(TURSO_SEARCH_FTS_SCHEMA);
      await connection.get(
        "SELECT fts_match(name, path, content, tags, metadata_text, '') AS matched FROM search_docs LIMIT 1",
      );
      this.nativeFullTextSearch = true;
    } catch {
      this.nativeFullTextSearch = false;
    }
    try {
      const row = await connection.get<{ distance: number }>(
        "SELECT vector_distance_cos(vector32('[1,0]'), vector32('[1,0]')) AS distance",
      );
      this.nativeVectorSearch = Math.abs(Number(row?.distance)) < 0.000001;
    } catch {
      this.nativeVectorSearch = false;
    }
  }

  async close(): Promise<void> {
    if (this.closeTask) return this.closeTask;
    const connection = this.connection;
    if (!connection) return;
    this.closeTask = (async () => {
      await this.commitQueue;
      await this.searchEmbeddingProvider?.dispose?.();
      this.searchEmbeddingProvider = null;
      await connection.close();
      this.connection = null;
      this.opened = false;
      this.changeListeners.clear();
    })();
    try {
      await this.closeTask;
    } finally {
      this.closeTask = null;
    }
  }

  async beginSearchIndexingBatch(): Promise<void> {}

  async endSearchIndexingBatch(): Promise<void> {}

  async configureSearchEmbeddingProvider(
    provider: SearchEmbeddingProviderConfig | null,
  ): Promise<void> {
    const pending = this.commitQueue.then(() =>
      this.configureSearchProviderImmediately(provider),
    );
    this.commitQueue = pending.then(
      () => undefined,
      () => undefined,
    );
    return pending;
  }

  private async configureSearchProviderImmediately(
    provider: SearchEmbeddingProviderConfig | null,
  ): Promise<void> {
    if (
      JSON.stringify(provider) ===
      JSON.stringify(this.searchEmbeddingProviderConfig)
    )
      return;
    const previous = this.searchEmbeddingProvider;
    const next = createSearchEmbeddingProvider(provider);
    try {
      const statements: TursoStatementInput[] = [
        {
          sql: `INSERT INTO app_meta (key, value_json) VALUES (?, ?)
              ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json`,
          args: [SEARCH_EMBEDDING_PROVIDER_META_KEY, JSON.stringify(provider)],
        },
      ];
      if (next)
        for (const document of await this.listSearchDocuments()) {
          statements.push(
            ...this.searchDocumentStatements(
              rebindSearchDocument(document, next.config),
            ),
          );
        }
      await this.commitImmediately(
        statements,
        ["search"],
        [],
        undefined,
        () => {
          this.searchEmbeddingProviderConfig = clone(provider);
          this.searchEmbeddingProvider = next;
        },
      );
    } catch (error) {
      if (this.searchEmbeddingProvider === next) await previous?.dispose?.();
      else await next?.dispose?.();
      throw error;
    }
    await previous?.dispose?.();
  }

  async getSearchEmbeddingProvider(): Promise<SearchEmbeddingProviderConfig | null> {
    return clone(this.searchEmbeddingProviderConfig);
  }

  async getSearchEmbeddingRuntimeStatus(): Promise<SearchEmbeddingRuntimeStatus | null> {
    return this.searchEmbeddingProvider
      ? clone(this.searchEmbeddingProvider.getRuntimeStatus())
      : null;
  }

  async getSearchIndexStats(): Promise<AppDatabaseSearchIndexStats> {
    const rows = await this.requireConnection().all<{
      embedding_json: string | null;
    }>("SELECT embedding_json FROM search_chunks");
    const stats: AppDatabaseSearchIndexStats = {
      documentCount: Number(
        (
          await this.requireConnection().get<{ count: number }>(
            "SELECT count(*) AS count FROM search_docs",
          )
        )?.count ?? 0,
      ),
      chunkCount: rows.length,
      readyChunkCount: 0,
      pendingChunkCount: 0,
      errorChunkCount: 0,
      lastError: null,
    };
    for (const row of rows) {
      const embedding = parseJson<{ status?: string; error?: string } | null>(
        row.embedding_json,
        null,
      );
      if (embedding?.status === "ready") stats.readyChunkCount += 1;
      else if (embedding?.status === "error") {
        stats.errorChunkCount += 1;
        stats.lastError ??= embedding.error ?? null;
      } else stats.pendingChunkCount += 1;
    }
    return stats;
  }

  async getMeta<T = unknown>(key: string): Promise<T | undefined> {
    const row = await this.requireConnection().get<{ value_json: string }>(
      "SELECT value_json FROM app_meta WHERE key = ?",
      key,
    );
    return row
      ? parseJson<T | undefined>(row.value_json, undefined)
      : undefined;
  }

  async setMeta(key: string, value: unknown): Promise<void> {
    if (value === undefined) {
      await this.commit(
        [{ sql: "DELETE FROM app_meta WHERE key = ?", args: [key] }],
        ["meta"],
      );
      return;
    }
    await this.commit(
      [
        {
          sql: `INSERT INTO app_meta (key, value_json) VALUES (?, ?)
              ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json`,
          args: [key, JSON.stringify(value)],
        },
      ],
      ["meta"],
    );
  }

  async scanMeta<T = unknown>({
    prefix,
    after,
    limit = 100,
  }: AppDatabaseMetaScanQuery): Promise<AppDatabaseMetaScanPage<T>> {
    const boundedLimit = Math.max(1, Math.min(500, Math.floor(limit)));
    const rows = await this.requireConnection().all<{
      key: string;
      value_json: string;
    }>(
      `SELECT key, value_json FROM app_meta
       WHERE substr(key, 1, ?) = ? AND key > ?
       ORDER BY key ASC LIMIT ?`,
      prefix.length,
      prefix,
      after ?? prefix,
      boundedLimit + 1,
    );
    const pageRows = rows.slice(0, boundedLimit);
    return {
      entries: pageRows.map((row) => ({
        key: row.key,
        value: parseJson<T>(row.value_json, undefined as T),
      })),
      nextCursor:
        rows.length > pageRows.length ? pageRows.at(-1)?.key : undefined,
    };
  }

  async transactMeta({
    checks = [],
    mutations,
  }: AppDatabaseMetaTransaction): Promise<AppDatabaseMetaTransactionResult> {
    const committed = this.commitQueue.then(async () => {
      for (const check of checks) {
        const row = await this.requireConnection().get<{ value_json: string }>(
          "SELECT value_json FROM app_meta WHERE key = ?",
          check.key,
        );
        if (check.exists !== undefined && Boolean(row) !== check.exists) {
          return { applied: false };
        }
        if (
          "value" in check &&
          JSON.stringify(
            row ? parseJson(row.value_json, undefined) : undefined,
          ) !== JSON.stringify(check.value)
        ) {
          return { applied: false };
        }
      }
      const statements: TursoStatementInput[] = mutations.map((mutation) =>
        mutation.delete || mutation.value === undefined
          ? { sql: "DELETE FROM app_meta WHERE key = ?", args: [mutation.key] }
          : {
              sql: `INSERT INTO app_meta (key, value_json) VALUES (?, ?)
                    ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json`,
              args: [mutation.key, JSON.stringify(mutation.value)],
            },
      );
      if (statements.length > 0) {
        await this.commitImmediately(statements, ["meta"]);
      }
      return { applied: true };
    });
    this.commitQueue = committed.then(
      () => undefined,
      () => undefined,
    );
    return committed;
  }

  async getNotebookState(
    sourcePath: string,
  ): Promise<AppDatabaseNotebookState | undefined> {
    return this.getMeta(notebookStateMetaKey(sourcePath));
  }

  async setNotebookState(
    sourcePath: string,
    state: AppDatabaseNotebookState,
  ): Promise<void> {
    await this.commit(
      [
        {
          sql: `INSERT INTO app_meta (key, value_json) VALUES (?, ?)
              ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json`,
          args: [notebookStateMetaKey(sourcePath), JSON.stringify(state)],
        },
      ],
      ["notebook"],
      [sourcePath],
    );
  }

  async deleteNotebookState(sourcePath: string): Promise<void> {
    await this.commit(
      [
        {
          sql: "DELETE FROM app_meta WHERE key = ?",
          args: [notebookStateMetaKey(sourcePath)],
        },
      ],
      ["notebook"],
      [sourcePath],
    );
  }

  async loadMetadataSnapshot(): Promise<MetadataCacheSnapshot | null> {
    const stored = await this.getMeta<MetadataCacheSnapshot>(
      METADATA_SNAPSHOT_META_KEY,
    );
    if (stored) return stored;
    const legacy = await this.requireConnection().get<{ state_json: string }>(
      "SELECT state_json FROM app_state WHERE id = 1",
    );
    return (
      parseJson<{ metadataSnapshot?: MetadataCacheSnapshot | null }>(
        legacy?.state_json,
        {},
      ).metadataSnapshot ?? null
    );
  }

  async saveMetadataSnapshot(snapshot: MetadataCacheSnapshot): Promise<void> {
    await this.commit(
      [
        {
          sql: `INSERT INTO app_meta (key, value_json) VALUES (?, ?)
              ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json`,
          args: [METADATA_SNAPSHOT_META_KEY, JSON.stringify(snapshot)],
        },
      ],
      ["metadata"],
    );
  }

  async getFileHistory(path: string): Promise<AppDatabaseFileHistory | null> {
    const pathRow = await this.requireConnection().get<{ file_id: string }>(
      "SELECT file_id FROM history_file_paths WHERE path = ?",
      path,
    );
    if (!pathRow) return null;
    const [fileRow, revisionRows] = await Promise.all([
      this.requireConnection().get<{ data_json: string }>(
        "SELECT data_json FROM history_files WHERE file_id = ?",
        pathRow.file_id,
      ),
      this.requireConnection().all<{ data_json: string }>(
        "SELECT data_json FROM history_revisions WHERE file_id = ? ORDER BY ordinal",
        pathRow.file_id,
      ),
    ]);
    if (!fileRow) return null;
    return {
      file: parseJson<AppDatabaseFileHistoryFile>(fileRow.data_json, {
        fileId: pathRow.file_id,
        currentPath: path,
        deleted: false,
      }),
      revisions: revisionRows.map((row) =>
        parseJson<AppDatabaseFileHistoryRevision>(
          row.data_json,
          {} as AppDatabaseFileHistoryRevision,
        ),
      ),
    };
  }

  async storeFileHistoryRevision(
    input: AppDatabaseStoreFileHistoryRevisionInput,
  ): Promise<AppDatabaseStoreFileHistoryRevisionResult> {
    const existing =
      (await this.getFileHistory(input.path)) ??
      (input.previousPath
        ? await this.getFileHistory(input.previousPath)
        : null);
    const fileId = existing?.file.fileId ?? createStableId("history-file");
    const latest = existing?.revisions.at(-1);
    const content = input.content ?? latest?.content ?? "";
    const contentHash =
      input.contentHash ?? latest?.contentHash ?? hashText(content);
    const deduplicate =
      input.eventType !== "rename" &&
      input.eventType !== "delete" &&
      !existing?.file.deleted &&
      latest?.contentHash === contentHash;
    if (deduplicate) return { fileId, stored: false, deduplicated: true };
    const file: AppDatabaseFileHistoryFile = {
      fileId,
      currentPath: input.path,
      deleted: input.eventType === "delete",
    };
    const revision: AppDatabaseFileHistoryRevision =
      input.replaceLatest && latest
        ? {
            ...latest,
            currentPath: input.path,
            capturedPath:
              input.eventType === "rename" && input.previousPath
                ? input.previousPath
                : input.path,
            eventType: input.eventType,
            createdAt: input.createdAt,
            sourceMtime: input.sourceMtime,
            sourceSize: input.sourceSize,
            contentHash,
            content,
          }
        : {
            revisionId: createStableId("history-revision"),
            fileId,
            currentPath: input.path,
            capturedPath:
              input.eventType === "rename" && input.previousPath
                ? input.previousPath
                : input.path,
            eventType: input.eventType,
            createdAt: input.createdAt,
            sourceMtime: input.sourceMtime,
            sourceSize: input.sourceSize,
            contentHash,
            content,
          };
    const revisions =
      input.replaceLatest && latest
        ? [...(existing?.revisions.slice(0, -1) ?? []), revision]
        : [...(existing?.revisions ?? []), revision];
    const retained = revisions.slice(
      -Math.max(1, input.maxRevisions ?? revisions.length),
    );
    const statements: TursoStatementInput[] = [
      {
        sql: `INSERT INTO history_files (file_id, data_json) VALUES (?, ?)
              ON CONFLICT(file_id) DO UPDATE SET data_json = excluded.data_json`,
        args: [fileId, JSON.stringify(file)],
      },
      {
        sql: "DELETE FROM history_revisions WHERE file_id = ?",
        args: [fileId],
      },
      {
        sql: "DELETE FROM history_file_paths WHERE file_id = ?",
        args: [fileId],
      },
      {
        sql: `INSERT INTO history_file_paths (path, file_id) VALUES (?, ?)
              ON CONFLICT(path) DO UPDATE SET file_id = excluded.file_id`,
        args: [input.path, fileId],
      },
    ];
    retained.forEach((entry, ordinal) =>
      statements.push({
        sql: "INSERT INTO history_revisions (file_id, ordinal, data_json) VALUES (?, ?, ?)",
        args: [fileId, ordinal, JSON.stringify(entry)],
      }),
    );
    await this.commit(statements, ["history"], [input.path]);
    return { fileId, stored: true, deduplicated: false, revision };
  }

  async listNotifications(): Promise<AppDatabaseNotificationRecord[]> {
    const rows = await this.requireConnection().all<{ data_json: string }>(
      "SELECT data_json FROM notifications",
    );
    return rows
      .map((row) =>
        parseJson<AppDatabaseNotificationRecord>(
          row.data_json,
          {} as AppDatabaseNotificationRecord,
        ),
      )
      .filter((record) => !record.cleared)
      .sort((left, right) => right.createdAt - left.createdAt);
  }

  async upsertNotification(
    record: AppDatabaseNotificationRecord,
  ): Promise<void> {
    await this.commit(
      [
        {
          sql: `INSERT INTO notifications (id, data_json) VALUES (?, ?)
            ON CONFLICT(id) DO UPDATE SET data_json = excluded.data_json`,
          args: [record.id, JSON.stringify(record)],
        },
      ],
      ["notification"],
    );
  }

  async markNotificationRead(id: string): Promise<void> {
    await this.updateNotification(id, (record) => ({
      ...record,
      read: true,
      updatedAt: Date.now(),
    }));
  }

  async clearNotification(id: string): Promise<void> {
    await this.updateNotification(id, (record) => ({
      ...record,
      cleared: true,
      updatedAt: Date.now(),
    }));
  }

  async clearAllNotifications(): Promise<void> {
    const rows = await this.requireConnection().all<{
      id: string;
      data_json: string;
    }>("SELECT id, data_json FROM notifications");
    const now = Date.now();
    await this.commit(
      rows.map((row) => ({
        sql: "UPDATE notifications SET data_json = ? WHERE id = ?",
        args: [
          JSON.stringify({
            ...parseJson(row.data_json, {}),
            cleared: true,
            updatedAt: now,
          }),
          row.id,
        ],
      })),
      ["notification"],
    );
  }

  async getMemorySourceState(
    sourceKey: string,
  ): Promise<AppDatabaseMemorySourceRecord | undefined> {
    const row = await this.requireConnection().get<{ data_json: string }>(
      "SELECT data_json FROM memory_source_state WHERE source_key = ?",
      sourceKey,
    );
    return row
      ? parseJson<AppDatabaseMemorySourceRecord>(
          row.data_json,
          {} as AppDatabaseMemorySourceRecord,
        )
      : undefined;
  }

  async listMemorySourceStates(): Promise<AppDatabaseMemorySourceRecord[]> {
    const rows = await this.requireConnection().all<{ data_json: string }>(
      "SELECT data_json FROM memory_source_state ORDER BY source_key",
    );
    return rows.map((row) =>
      parseJson<AppDatabaseMemorySourceRecord>(
        row.data_json,
        {} as AppDatabaseMemorySourceRecord,
      ),
    );
  }

  async upsertMemorySourceState(
    record: AppDatabaseMemorySourceRecord,
  ): Promise<void> {
    await this.commit(
      [
        {
          sql: `INSERT INTO memory_source_state
              (source_key, source_path, source_hash, status, indexed_at, data_json)
              VALUES (?, ?, ?, ?, ?, ?)
              ON CONFLICT(source_key) DO UPDATE SET
                source_path = excluded.source_path,
                source_hash = excluded.source_hash,
                status = excluded.status,
                indexed_at = excluded.indexed_at,
                data_json = excluded.data_json`,
          args: [
            record.sourceKey,
            record.sourcePath,
            record.sourceHash,
            record.status,
            record.indexedAt,
            JSON.stringify(record),
          ],
        },
      ],
      ["memory"],
      [record.sourcePath],
    );
  }

  async deleteMemorySourceState(sourceKey: string): Promise<void> {
    const existing = await this.getMemorySourceState(sourceKey);
    await this.commit(
      [
        {
          sql: "DELETE FROM memory_source_state WHERE source_key = ?",
          args: [sourceKey],
        },
      ],
      ["memory"],
      existing ? [existing.sourcePath] : [],
    );
  }

  async upsertMemoryCandidate(
    input: AppDatabaseMemoryCandidateInput,
  ): Promise<void> {
    const candidate = input.candidate;
    await this.commit(
      [
        {
          sql: `INSERT INTO memory_candidates
              (id, scope_kind, scope_path, state, last_seen_at, data_json)
              VALUES (?, ?, ?, ?, ?, ?)
              ON CONFLICT(id) DO UPDATE SET
                scope_kind = excluded.scope_kind,
                scope_path = excluded.scope_path,
                state = excluded.state,
                last_seen_at = excluded.last_seen_at,
                data_json = excluded.data_json`,
          args: [
            candidate.id,
            candidate.scopeKind,
            candidate.scopePath,
            candidate.state,
            candidate.lastSeenAt,
            JSON.stringify(candidate),
          ],
        },
        {
          sql: "DELETE FROM memory_candidate_origins WHERE candidate_id = ?",
          args: [candidate.id],
        },
        ...input.origins
          .filter((origin) => origin.candidateId === candidate.id)
          .map((origin) => ({
            sql: `INSERT INTO memory_candidate_origins
                (candidate_id, conversation_id, entry_id, data_json)
                VALUES (?, ?, ?, ?)`,
            args: [
              origin.candidateId,
              origin.conversationId,
              origin.entryId,
              JSON.stringify(origin),
            ],
          })),
      ],
      ["memory"],
    );
  }

  async queryMemoryCandidates(
    query: AppDatabaseMemoryCandidateQuery = {},
  ): Promise<AppDatabaseMemoryCandidateResult[]> {
    const clauses: string[] = [];
    const args: unknown[] = [];
    if (query.scopeKind) {
      clauses.push("scope_kind = ?");
      args.push(query.scopeKind);
    }
    if (query.scopePath !== undefined) {
      clauses.push("scope_path = ?");
      args.push(query.scopePath);
    }
    if (query.states?.length) {
      clauses.push(`state IN (${query.states.map(() => "?").join(", ")})`);
      args.push(...query.states);
    }
    const limit = Math.max(1, Math.min(query.limit ?? 1_000, 10_000));
    const rows = await this.requireConnection().all<{
      id: string;
      data_json: string;
    }>(
      `SELECT id, data_json FROM memory_candidates
       ${clauses.length ? `WHERE ${clauses.join(" AND ")}` : ""}
       ORDER BY last_seen_at DESC, id ASC LIMIT ?`,
      ...args,
      limit,
    );
    if (rows.length === 0) return [];
    const originRows = await this.requireConnection().all<{
      candidate_id: string;
      data_json: string;
    }>(
      `SELECT candidate_id, data_json FROM memory_candidate_origins
       WHERE candidate_id IN (${rows.map(() => "?").join(", ")})
       ORDER BY candidate_id, conversation_id, entry_id`,
      ...rows.map((row) => row.id),
    );
    const origins = new Map<string, AppDatabaseMemoryCandidateOriginRecord[]>();
    for (const row of originRows) {
      const values = origins.get(row.candidate_id) ?? [];
      values.push(
        parseJson<AppDatabaseMemoryCandidateOriginRecord>(
          row.data_json,
          {} as AppDatabaseMemoryCandidateOriginRecord,
        ),
      );
      origins.set(row.candidate_id, values);
    }
    return rows.map((row) => ({
      candidate: parseJson<AppDatabaseMemoryCandidateRecord>(
        row.data_json,
        {} as AppDatabaseMemoryCandidateRecord,
      ),
      origins: origins.get(row.id) ?? [],
    }));
  }

  async deleteMemoryCandidatesByConversation(
    conversationId: string,
  ): Promise<void> {
    const rows = await this.requireConnection().all<{ candidate_id: string }>(
      `SELECT DISTINCT candidate_id FROM memory_candidate_origins
       WHERE conversation_id = ?`,
      conversationId,
    );
    if (rows.length === 0) return;
    const ids = rows.map((row) => row.candidate_id);
    const placeholders = ids.map(() => "?").join(", ");
    await this.commit(
      [
        {
          sql: `DELETE FROM memory_candidate_origins
                WHERE candidate_id IN (${placeholders})`,
          args: ids,
        },
        {
          sql: `DELETE FROM memory_candidates WHERE id IN (${placeholders})`,
          args: ids,
        },
      ],
      ["memory"],
    );
  }

  async recordMemoryRecallSignal(
    record: AppDatabaseMemoryRecallSignalRecord,
  ): Promise<void> {
    await this.commit(
      [
        {
          sql: `INSERT INTO memory_recall_signals
              (target_ref, query_fingerprint, day, best_score, hit_count, data_json)
              VALUES (?, ?, ?, ?, ?, ?)
              ON CONFLICT(target_ref, query_fingerprint, day) DO UPDATE SET
                best_score = max(best_score, excluded.best_score),
                hit_count = hit_count + excluded.hit_count,
                data_json = excluded.data_json`,
          args: [
            record.targetRef,
            record.queryFingerprint,
            record.day,
            record.bestScore,
            Math.max(0, record.hitCount),
            JSON.stringify(record),
          ],
        },
      ],
      ["memory"],
    );
  }

  async listMemoryRecallSignals(
    targetRef?: string,
  ): Promise<AppDatabaseMemoryRecallSignalRecord[]> {
    const rows = await this.requireConnection().all<{
      target_ref: string;
      query_fingerprint: string;
      day: string;
      best_score: number;
      hit_count: number;
      data_json: string;
    }>(
      `SELECT target_ref, query_fingerprint, day, best_score, hit_count, data_json
       FROM memory_recall_signals
       ${targetRef === undefined ? "" : "WHERE target_ref = ?"}
       ORDER BY day DESC, query_fingerprint`,
      ...(targetRef === undefined ? [] : [targetRef]),
    );
    return rows.map((row) => ({
      ...parseJson<AppDatabaseMemoryRecallSignalRecord>(row.data_json, {
        targetRef: row.target_ref,
        queryFingerprint: row.query_fingerprint,
        day: row.day,
        bestScore: Number(row.best_score),
        hitCount: Number(row.hit_count),
      }),
      bestScore: Number(row.best_score),
      hitCount: Number(row.hit_count),
    }));
  }

  async claimMemoryJob(
    input: AppDatabaseMemoryJobClaimInput,
  ): Promise<AppDatabaseMemoryJobRecord | null> {
    const row = await this.requireConnection().get<{ data_json: string }>(
      "SELECT data_json FROM memory_jobs WHERE id = ?",
      input.job.id,
    );
    const existing = row
      ? parseJson<AppDatabaseMemoryJobRecord>(
          row.data_json,
          {} as AppDatabaseMemoryJobRecord,
        )
      : undefined;
    if (existing?.status === "completed") return null;
    const renewing =
      existing?.status === "running" && existing.ownerId === input.ownerId;
    if (
      existing?.status === "running" &&
      existing.ownerId !== input.ownerId &&
      (existing.leaseUntil ?? 0) > input.now
    ) {
      return null;
    }
    const base = existing ?? input.job;
    const attempts = renewing ? base.attempts : base.attempts + 1;
    if (attempts > base.maxAttempts) return null;
    const claimed: AppDatabaseMemoryJobRecord = {
      ...base,
      status: "running",
      attempts,
      ownerId: input.ownerId,
      leaseUntil: input.now + Math.max(1, input.leaseMs),
      startedAt: renewing ? base.startedAt : input.now,
      finishedAt: undefined,
      errorCode: undefined,
    };
    await this.commit(
      [
        {
          sql: `INSERT INTO memory_jobs
              (id, scope_key, status, lease_until, created_at, data_json)
              VALUES (?, ?, ?, ?, ?, ?)
              ON CONFLICT(id) DO UPDATE SET
                scope_key = excluded.scope_key,
                status = excluded.status,
                lease_until = excluded.lease_until,
                created_at = excluded.created_at,
                data_json = excluded.data_json`,
          args: [
            claimed.id,
            claimed.scopeKey,
            claimed.status,
            claimed.leaseUntil ?? null,
            claimed.createdAt,
            JSON.stringify(claimed),
          ],
        },
      ],
      ["memory"],
    );
    return claimed;
  }

  async updateMemoryJob(
    input: AppDatabaseMemoryJobUpdateInput,
  ): Promise<AppDatabaseMemoryJobRecord | null> {
    const row = await this.requireConnection().get<{ data_json: string }>(
      "SELECT data_json FROM memory_jobs WHERE id = ?",
      input.jobId,
    );
    if (!row) return null;
    const existing = parseJson<AppDatabaseMemoryJobRecord>(
      row.data_json,
      {} as AppDatabaseMemoryJobRecord,
    );
    if (
      existing.status !== "running" ||
      existing.ownerId !== input.ownerId ||
      (existing.leaseUntil ?? 0) < input.now
    ) {
      return null;
    }
    const updated: AppDatabaseMemoryJobRecord = {
      ...existing,
      ...input.patch,
      ownerId: input.patch.status === "running" ? existing.ownerId : undefined,
      leaseUntil:
        input.patch.status === "running"
          ? (input.patch.leaseUntil ?? existing.leaseUntil)
          : undefined,
    };
    await this.commit(
      [
        {
          sql: `UPDATE memory_jobs
                SET status = ?, lease_until = ?, data_json = ? WHERE id = ?`,
          args: [
            updated.status,
            updated.leaseUntil ?? null,
            JSON.stringify(updated),
            updated.id,
          ],
        },
      ],
      ["memory"],
    );
    return updated;
  }

  async listMemoryJobs(
    scopeKey?: string,
  ): Promise<AppDatabaseMemoryJobRecord[]> {
    const rows = await this.requireConnection().all<{ data_json: string }>(
      `SELECT data_json FROM memory_jobs
       ${scopeKey === undefined ? "" : "WHERE scope_key = ?"}
       ORDER BY created_at DESC, id`,
      ...(scopeKey === undefined ? [] : [scopeKey]),
    );
    return rows.map((row) =>
      parseJson<AppDatabaseMemoryJobRecord>(
        row.data_json,
        {} as AppDatabaseMemoryJobRecord,
      ),
    );
  }

  async clearMemoryDerivedState(): Promise<void> {
    await this.commit(
      [
        "DELETE FROM memory_candidate_origins",
        "DELETE FROM memory_candidates",
        "DELETE FROM memory_source_state",
        "DELETE FROM memory_recall_signals",
        "DELETE FROM memory_jobs",
      ],
      ["memory"],
    );
  }

  async getChangeRevision(): Promise<number> {
    return this.currentRevision;
  }

  subscribeToChanges(listener: AppDatabaseChangeListener): () => void {
    this.changeListeners.add(listener);
    return () => this.changeListeners.delete(listener);
  }

  async upsertIndexedFile(record: AppDatabaseIndexedFile): Promise<void> {
    await this.commit(
      indexedFileStatements(record),
      ["metadata"],
      [record.file.path],
    );
    if (record.task) await this.upsertTaskProjection(record.task);
    else if (record.task === null)
      await this.deleteTaskProjection(record.file.path);
  }

  async getIndexedFile(
    path: string,
  ): Promise<AppDatabaseIndexedMetadataRow | undefined> {
    return this.readIndexedFile(path, false);
  }

  async listIndexedFileManifest(
    query: AppDatabaseIndexedFileManifestQuery = {},
  ): Promise<AppDatabaseIndexedFileManifestPage> {
    const limit = Math.max(1, query.limit ?? 500);
    const paths = [...new Set(query.paths ?? [])];
    const pathFilter = paths.length
      ? `AND f.path IN (${paths.map(() => "?").join(", ")})`
      : "";
    const rows = await this.requireConnection().all<Record<string, unknown>>(
      `SELECT f.path, f.normalized_path, f.extension, f.mtime, f.size, f.hash,
              f.indexed, f.deleted, m.parser_version
       FROM files f
       LEFT JOIN metadata m ON m.path = f.path
       WHERE f.indexed = 1 AND f.deleted = 0 AND f.path > ? ${pathFilter}
       ORDER BY f.path LIMIT ?`,
      query.after ?? "",
      ...paths,
      limit + 1,
    );
    const page = rows.slice(0, limit).map((row) => this.fileRecordFromRow(row));
    return {
      rows: page,
      nextCursor: rows.length > limit ? page.at(-1)?.path : undefined,
    };
  }

  async queryIndexedMetadata(
    query: AppDatabaseIndexedMetadataQuery = {},
  ): Promise<AppDatabaseIndexedMetadataRow[]> {
    return (
      await this.queryIndexedMetadataPage({
        query,
        limit: query.limit ?? 10_000,
      })
    ).rows;
  }

  async queryIndexedMetadataPage(
    input: AppDatabaseIndexedMetadataPageQuery = {},
  ): Promise<AppDatabaseIndexedMetadataPage> {
    const limit = Math.max(1, input.limit ?? input.query?.limit ?? 100);
    const { sql, args } = this.compileMetadataPathQuery(
      input.query ?? {},
      input.after,
      limit + 1,
    );
    const rows = await this.requireConnection().all<{ path: string }>(
      sql,
      ...args,
    );
    const pagePaths = rows.slice(0, limit).map((row) => row.path);
    const materialized = await this.readIndexedFiles(pagePaths, input.include);
    return {
      rows: materialized,
      nextCursor: rows.length > limit ? pagePaths.at(-1) : undefined,
    };
  }

  async queryMetadataFacets(
    query: AppDatabaseMetadataFacetQuery,
  ): Promise<AppDatabaseMetadataFacetRow[]> {
    const limit = Math.max(1, query.limit ?? 100);
    const prefixArgs: unknown[] = [];
    const prefix = this.compilePathPrefixes(
      "path",
      query.pathPrefixes,
      prefixArgs,
    );
    if (query.kind === "tag") {
      const rows = await this.requireConnection().all<{
        value: string;
        count: number;
      }>(
        `SELECT ancestor AS value, count(DISTINCT path) AS count
         FROM metadata_tag_ancestors
         WHERE 1 = 1 ${prefix}
         GROUP BY ancestor ORDER BY count DESC, ancestor LIMIT ?`,
        ...prefixArgs,
        limit,
      );
      return rows.map((row) => ({
        value: row.value,
        valueType: "string",
        count: Number(row.count),
      }));
    }
    if (query.kind === "property-name") {
      const rows = await this.requireConnection().all<{
        value: string;
        count: number;
        metadata_types: string | null;
      }>(
        `SELECT min(name) AS value, count(DISTINCT path) AS count,
                group_concat(DISTINCT COALESCE(declared_type, inferred_type)) AS metadata_types
         FROM metadata_properties
         WHERE 1 = 1 ${prefix}
         GROUP BY normalized_name ORDER BY count DESC, normalized_name LIMIT ?`,
        ...prefixArgs,
        limit,
      );
      return rows.map((row) => ({
        value: row.value,
        valueType: "string",
        count: Number(row.count),
        metadataTypes: row.metadata_types?.split(",").filter(Boolean),
        topLevel: true,
      }));
    }
    if (query.kind === "property-path") {
      const rows = await this.requireConnection().all<Record<string, unknown>>(
        `SELECT min(value) AS value, count(DISTINCT path) AS count,
                group_concat(DISTINCT metadata_type) AS metadata_types,
                max(top_level) AS top_level
         FROM (
           SELECT property_path AS value, normalized_property_path AS normalized_value,
                  value_type AS metadata_type, path,
                  CASE WHEN property_name = normalized_property_path THEN 1 ELSE 0 END AS top_level
           FROM metadata_property_values
           UNION ALL
           SELECT name AS value, normalized_name AS normalized_value,
                  COALESCE(declared_type, inferred_type) AS metadata_type, path, 1 AS top_level
           FROM metadata_properties
         ) property_paths
         WHERE 1 = 1 ${prefix}
         GROUP BY normalized_value ORDER BY count DESC, normalized_value LIMIT ?`,
        ...prefixArgs,
        limit,
      );
      return rows.map((row) => ({
        value: String(row.value).replace(/\[\d+\]/gu, "[]"),
        valueType: "string",
        count: Number(row.count),
        metadataTypes: String(row.metadata_types ?? "")
          .split(",")
          .filter(Boolean),
        topLevel: Boolean(row.top_level),
      }));
    }
    if (!query.propertyName) return [];
    const rows = await this.requireConnection().all<Record<string, unknown>>(
      `SELECT value_type, text_value, number_value, boolean_value, date_value,
              count(DISTINCT path) AS count
       FROM metadata_property_values
       WHERE (property_name = ? OR normalized_property_path = ?) ${prefix}
       GROUP BY value_type, text_value, number_value, boolean_value, date_value
       ORDER BY count DESC LIMIT ?`,
      normalizePropertyPath(query.propertyName),
      normalizePropertyPath(query.propertyName),
      ...prefixArgs,
      limit,
    );
    return rows.map((row) => ({
      value:
        row.value_type === "number"
          ? Number(row.number_value)
          : row.value_type === "boolean"
            ? Boolean(row.boolean_value)
            : row.value_type === "null"
              ? null
              : String(row.date_value ?? row.text_value ?? ""),
      valueType: String(
        row.value_type,
      ) as AppDatabaseMetadataFacetRow["valueType"],
      count: Number(row.count),
    }));
  }

  async queryMetadataLinks(
    query: AppDatabaseMetadataLinkQuery,
  ): Promise<AppDatabaseLinkRecord[]> {
    const paths = [
      ...new Set([...(query.paths ?? []), ...(query.path ? [query.path] : [])]),
    ];
    if (!paths.length) return [];
    const field =
      query.direction === "outgoing" ? "source_path" : "resolved_target_path";
    const conditions = [`${field} IN (${paths.map(() => "?").join(", ")})`];
    if (query.resolution === "resolved")
      conditions.push("resolution_state = 'resolved'");
    if (query.resolution === "unresolved")
      conditions.push("resolution_state = 'unresolved'");
    const rows = await this.requireConnection().all<{ data_json: string }>(
      `SELECT data_json FROM metadata_links WHERE ${conditions.join(" AND ")}
       ORDER BY source_path, ordinal LIMIT ?`,
      ...paths,
      query.limit ?? 1000,
    );
    return rows.map((row) =>
      parseJson<AppDatabaseLinkRecord>(
        row.data_json,
        {} as AppDatabaseLinkRecord,
      ),
    );
  }

  async deleteIndexedFile(path: string): Promise<void> {
    await this.commit(
      [
        {
          sql: "UPDATE files SET indexed = 0, deleted = 1 WHERE path = ?",
          args: [path],
        },
        ...this.deletePathStatements(path),
      ],
      ["metadata", "search", "task", "projection", "notebook"],
      [path],
    );
  }

  async renameIndexedFile(oldPath: string, newPath: string): Promise<void> {
    const record = await this.readIndexedFile(oldPath, false);
    if (!record) return;
    const renamed: AppDatabaseIndexedFile = {
      file: { ...record.file, path: newPath, normalizedPath: newPath },
      metadata: record.metadata
        ? { ...record.metadata, path: newPath }
        : {
            path: newPath,
            hash: record.file.hash,
            parserVersion: "unknown",
            metadata: {},
          },
      links: record.links.map((link) => ({ ...link, sourcePath: newPath })),
      tags: record.tags.map((tag) => ({ ...tag, path: newPath })),
      properties: record.properties.map((property) => ({
        ...property,
        path: newPath,
      })),
    };
    const search = await this.getSearchDocument(oldPath);
    const notebook = await this.getNotebookState(oldPath);
    const task = await this.getTaskRow({ path: oldPath });
    const incoming = await this.requireConnection().all<{
      source_path: string;
      ordinal: number;
      data_json: string;
    }>(
      "SELECT source_path, ordinal, data_json FROM metadata_links WHERE resolved_target_path = ?",
      oldPath,
    );
    const statements = [
      ...this.deleteMetadataPathStatements(oldPath),
      { sql: "DELETE FROM files WHERE path = ?", args: [oldPath] },
      ...indexedFileStatements(renamed),
      {
        sql: "UPDATE index_projection_sources SET source_path = ? WHERE source_path = ?",
        args: [newPath, oldPath],
      },
      {
        sql: "UPDATE index_projection_rows SET source_path = ? WHERE source_path = ?",
        args: [newPath, oldPath],
      },
      {
        sql: "UPDATE index_projection_edges SET target_path = ? WHERE target_path = ?",
        args: [newPath, oldPath],
      },
    ] satisfies TursoStatementInput[];
    if (notebook) {
      statements.push(
        {
          sql: "DELETE FROM app_meta WHERE key = ?",
          args: [notebookStateMetaKey(oldPath)],
        },
        {
          sql: `INSERT INTO app_meta (key, value_json) VALUES (?, ?)
                ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json`,
          args: [
            notebookStateMetaKey(newPath),
            JSON.stringify({ ...notebook, sourcePath: newPath }),
          ],
        },
      );
    }
    if (search)
      statements.push(
        ...this.searchDocumentStatements(
          {
            ...search,
            path: newPath,
            chunks: search.chunks?.map((chunk) => ({
              ...chunk,
              id:
                chunk.id === oldPath
                  ? newPath
                  : chunk.id.startsWith(`${oldPath}#`)
                    ? `${newPath}${chunk.id.slice(oldPath.length)}`
                    : chunk.id,
            })),
          },
          oldPath,
        ),
      );
    if (task)
      statements.push(
        {
          sql: "DELETE FROM task_records WHERE document_path = ?",
          args: [oldPath],
        },
        {
          sql: `INSERT INTO task_records (document_path, data_json) VALUES (?, ?)
              ON CONFLICT(document_path) DO UPDATE SET data_json = excluded.data_json`,
          args: [newPath, JSON.stringify({ ...task, documentPath: newPath })],
        },
      );
    for (const row of incoming) {
      const link = parseJson<AppDatabaseLinkRecord>(
        row.data_json,
        {} as AppDatabaseLinkRecord,
      );
      const data = JSON.stringify({ ...link, resolvedTargetPath: newPath });
      statements.push(
        {
          sql: "UPDATE metadata_links SET resolved_target_path = ?, data_json = ? WHERE source_path = ? AND ordinal = ?",
          args: [newPath, data, row.source_path, row.ordinal],
        },
        {
          sql: "UPDATE links SET data_json = ? WHERE source_path = ? AND ordinal = ?",
          args: [data, row.source_path, row.ordinal],
        },
      );
    }
    await this.commit(
      statements,
      ["metadata", "search", "task", "projection", "notebook"],
      [oldPath, newPath],
      [{ oldPath, newPath }],
    );
  }

  async upsertSearchDocument(document: SearchDocumentRecord): Promise<void> {
    const prepared = await this.prepareSearchDocument(document);
    await this.commit(
      this.searchDocumentStatements(prepared),
      ["search"],
      [prepared.path],
    );
  }

  async deleteSearchDocument(path: string): Promise<void> {
    await this.commit(
      [
        { sql: "DELETE FROM search_chunks WHERE path = ?", args: [path] },
        { sql: "DELETE FROM search_docs WHERE path = ?", args: [path] },
      ],
      ["search"],
      [path],
    );
  }

  async getSearchDocument(
    path: string,
  ): Promise<SearchDocumentRecord | undefined> {
    const row = await this.requireConnection().get<{ data_json: string }>(
      "SELECT data_json FROM search_docs WHERE path = ?",
      path,
    );
    return row
      ? parseJson<SearchDocumentRecord | undefined>(row.data_json, undefined)
      : undefined;
  }

  async listSearchDocumentManifest(
    query: SearchDocumentManifestQuery = {},
  ): Promise<SearchDocumentManifestPage> {
    const limit = Math.max(1, query.limit ?? 500);
    const rows = await this.requireConnection().all<Record<string, unknown>>(
      `SELECT path, checksum, source_provider_id, metadata_hash,
              provider_version, projection_signature, source_mtime, source_size
       FROM search_docs WHERE path > ? ORDER BY path LIMIT ?`,
      query.after ?? "",
      limit + 1,
    );
    const page = rows.slice(0, limit);
    return {
      rows: page.map((row) => ({
        path: String(row.path),
        checksum: String(row.checksum),
        sourceProviderId:
          row.source_provider_id == null
            ? undefined
            : String(row.source_provider_id),
        metadataHash:
          row.metadata_hash == null ? undefined : String(row.metadata_hash),
        providerVersion:
          row.provider_version == null
            ? undefined
            : String(row.provider_version),
        projectionSignature:
          row.projection_signature == null
            ? undefined
            : String(row.projection_signature),
        sourceMtime:
          row.source_mtime == null ? undefined : Number(row.source_mtime),
        sourceSize:
          row.source_size == null ? undefined : Number(row.source_size),
      })),
      nextCursor: rows.length > limit ? String(page.at(-1)?.path) : undefined,
    };
  }

  async listSearchDocuments(): Promise<SearchDocumentRecord[]> {
    const rows = await this.requireConnection().all<{ data_json: string }>(
      "SELECT data_json FROM search_docs ORDER BY path",
    );
    return rows.map((row) =>
      parseJson<SearchDocumentRecord>(
        row.data_json,
        {} as SearchDocumentRecord,
      ),
    );
  }

  async rebuildSearchIndex(): Promise<void> {
    const pageSize = 100;
    let after = "";
    let rebuilt = false;
    while (true) {
      const rows = await this.requireConnection().all<{
        path: string;
        data_json: string;
      }>(
        `SELECT path, data_json FROM search_docs
         WHERE path > ? ORDER BY path LIMIT ?`,
        after,
        pageSize + 1,
      );
      const page = rows.slice(0, pageSize);
      if (!page.length) break;

      const statements: TursoStatementInput[] = [];
      const paths: string[] = [];
      for (const row of page) {
        const document = parseJson<SearchDocumentRecord>(
          row.data_json,
          {} as SearchDocumentRecord,
        );
        const prepared = await this.prepareSearchDocument(document);
        statements.push(...this.searchDocumentStatements(prepared));
        paths.push(row.path);
      }
      await this.commit(statements, ["search"], paths);
      rebuilt = true;
      after = page.at(-1)?.path ?? after;
      if (rows.length <= pageSize) break;
    }
    if (!rebuilt) await this.commit([], ["search"]);
  }

  async searchDocuments(
    query: string,
    options: AppDatabaseSearchOptions = {},
  ): Promise<AppDatabaseSearchResult[]> {
    const requestedMode = options.mode ?? "auto";
    const allPathsRequired =
      options.caseSensitive ||
      isStructuredSearchQuery(query) ||
      Boolean(options.sourceProviderIds?.length) ||
      Boolean(options.pathPrefix);
    if (allPathsRequired) {
      return this.evaluateSearchDocumentsForPaths(
        query,
        options,
        await this.queryAllSearchPaths(options),
      );
    }

    const candidates = new Set<string>();
    const includesLexical = requestedMode !== "vector";
    const includesVector =
      requestedMode !== "lexical" &&
      this.nativeVectorSearch &&
      Boolean(this.searchEmbeddingProvider);

    if (includesLexical) {
      for (const path of await this.queryLexicalPaths(query, options.limit)) {
        candidates.add(path);
      }
    }
    if (includesVector) {
      const queryVector = await this.safeEmbedQuery(query);
      if (queryVector) {
        for (const path of await this.queryVectorPaths(
          queryVector,
          options.limit,
        )) {
          candidates.add(path);
        }
      }
    }

    if (requestedMode === "vector" && !includesVector) return [];
    return this.evaluateSearchDocumentsForPaths(query, options, candidates);
  }

  async searchDocumentPaths(
    query: string,
    options: AppDatabaseSearchOptions = {},
  ): Promise<string[]> {
    return (await this.searchDocuments(query, options)).map(
      (result) => result.document.path,
    );
  }

  private async queryAllSearchPaths(
    options: Pick<
      AppDatabaseSearchOptions,
      "pathPrefix" | "sourceProviderIds"
    > = {},
  ): Promise<string[]> {
    const clauses: string[] = [];
    const args: Array<string | number> = [];
    if (options.sourceProviderIds?.length) {
      clauses.push(
        `source_provider_id IN (${options.sourceProviderIds.map(() => "?").join(", ")})`,
      );
      args.push(...options.sourceProviderIds);
    }
    if (options.pathPrefix) {
      clauses.push("(path = ? OR instr(path, ?) = 1)");
      args.push(options.pathPrefix, `${options.pathPrefix}/`);
    }
    const rows = await this.requireConnection().all<{ path: string }>(
      `SELECT path FROM search_docs
       ${clauses.length > 0 ? `WHERE ${clauses.join(" AND ")}` : ""}
       ORDER BY path`,
      ...args,
    );
    return rows.map((row) => row.path);
  }

  private async queryLexicalPaths(
    query: string,
    limit = 100,
  ): Promise<string[]> {
    const terms = collectSearchQueryTerms(query);
    if (!terms.length || !this.nativeFullTextSearch) {
      return this.queryAllSearchPaths();
    }
    const ftsQuery = ftsQueryForTerms(terms);
    const rows = await this.requireConnection().all<{ path: string }>(
      `SELECT path,
              fts_score(name, path, content, tags, metadata_text, ?) AS score
       FROM search_docs
       WHERE fts_match(name, path, content, tags, metadata_text, ?)
       ORDER BY score ASC
       LIMIT ?`,
      ftsQuery,
      ftsQuery,
      limit,
    );
    return rows.map((row) => row.path);
  }

  private async queryVectorPaths(
    queryVector: number[],
    limit = 100,
  ): Promise<string[]> {
    const rows = await this.requireConnection().all<{ path: string }>(
      `SELECT path,
              vector_distance_cos(embedding, vector32(?)) AS distance
       FROM search_chunks
       WHERE embedding IS NOT NULL
       ORDER BY distance ASC
       LIMIT ?`,
      JSON.stringify(queryVector),
      limit,
    );
    return [...new Set(rows.map((row) => row.path))];
  }

  async upsertTaskProjection(record: AppDatabaseTaskRecord): Promise<void> {
    await this.ensureTasksProjectionDefinition();
    await this.commit(
      [
        {
          sql: `INSERT INTO task_records (document_path, data_json) VALUES (?, ?)
            ON CONFLICT(document_path) DO UPDATE SET data_json = excluded.data_json`,
          args: [record.documentPath, JSON.stringify(record)],
        },
      ],
      ["task"],
      [record.documentPath],
    );
    await this.replaceProjectionSource({
      projectionId: PUBLIC_TASKS_PROJECTION_ID,
      sourcePath: record.documentPath,
      sourceHash:
        (
          await this.requireConnection().get<{ hash: string }>(
            "SELECT hash FROM files WHERE path = ?",
            record.documentPath,
          )
        )?.hash ?? record.documentId,
      rows: [{ id: record.documentId, kind: record.kind, data: { ...record } }],
    });
  }

  async deleteTaskProjection(path: string): Promise<void> {
    await this.commit(
      [
        {
          sql: "DELETE FROM task_records WHERE document_path = ?",
          args: [path],
        },
        ...this.deleteProjectionSourceStatements(
          PUBLIC_TASKS_PROJECTION_ID,
          path,
        ),
      ],
      ["task", "projection"],
      [path],
    );
  }

  async queryTasks(
    query: AppDatabaseTaskQuery = {},
  ): Promise<AppDatabaseTaskRecord[]> {
    return (
      await this.queryProjection<AppDatabaseTaskRecord>(
        PUBLIC_TASKS_PROJECTION_ID,
        taskQueryToIndexQuery(query),
      )
    ).rows;
  }

  async getTaskRow(lookup: {
    path?: string;
    id?: string;
  }): Promise<AppDatabaseTaskRecord | undefined> {
    if (lookup.id)
      return (
        (await this.getProjectionRow<AppDatabaseTaskRecord>(
          PUBLIC_TASKS_PROJECTION_ID,
          lookup.id,
        )) ?? undefined
      );
    if (!lookup.path) return undefined;
    const row = await this.requireConnection().get<{ data_json: string }>(
      "SELECT data_json FROM task_records WHERE document_path = ?",
      lookup.path,
    );
    return row
      ? parseJson<AppDatabaseTaskRecord | undefined>(row.data_json, undefined)
      : undefined;
  }

  async listChildLinks(
    query: AppDatabaseTaskChildQuery,
  ): Promise<AppDatabaseLinkRecord[]> {
    const conditions = ["source_path = ?"];
    const args: unknown[] = [query.sourcePath];
    if (query.kind) {
      conditions.push("link_kind = ?");
      args.push(query.kind);
    }
    const rows = await this.requireConnection().all<{ data_json: string }>(
      `SELECT data_json FROM metadata_links WHERE ${conditions.join(" AND ")} ORDER BY ordinal`,
      ...args,
    );
    return rows.map((row) =>
      parseJson<AppDatabaseLinkRecord>(
        row.data_json,
        {} as AppDatabaseLinkRecord,
      ),
    );
  }

  async listTaskDescendants(path: string): Promise<AppDatabaseTaskRecord[]> {
    const source = await this.requireConnection().get<{ row_id: string }>(
      "SELECT row_id FROM index_projection_rows WHERE projection_id = ? AND source_path = ? LIMIT 1",
      PUBLIC_TASKS_PROJECTION_ID,
      path,
    );
    if (!source) return [];
    const seen = new Set([source.row_id]);
    const queue = [source.row_id];
    const result: AppDatabaseTaskRecord[] = [];
    while (queue.length) {
      const sourceRowId = queue.shift()!;
      const edges = await this.requireConnection().all<{
        target_row_id: string | null;
        target_path: string | null;
      }>(
        `SELECT target_row_id, target_path FROM index_projection_edges
         WHERE projection_id = ? AND source_row_id = ? AND relation IN ('task-entry', 'list-entry')
         ORDER BY ordinal`,
        PUBLIC_TASKS_PROJECTION_ID,
        sourceRowId,
      );
      for (const edge of edges) {
        const row = edge.target_row_id
          ? await this.getProjectionRow<AppDatabaseTaskRecord>(
              PUBLIC_TASKS_PROJECTION_ID,
              edge.target_row_id,
            )
          : edge.target_path
            ? await this.getTaskRow({ path: edge.target_path })
            : null;
        if (!row || seen.has(row.documentId)) continue;
        seen.add(row.documentId);
        queue.push(row.documentId);
        result.push(row);
      }
    }
    return result;
  }

  async registerProjectionDefinition(
    definition: IndexProjectionDefinitionRecord,
  ): Promise<void> {
    await this.commit(
      [
        {
          sql: `INSERT INTO index_projections
            (projection_id, owner_plugin_id, schema_version, config_hash, visibility, fields_json, active, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(projection_id) DO UPDATE SET
              owner_plugin_id = excluded.owner_plugin_id,
              schema_version = excluded.schema_version,
              config_hash = excluded.config_hash,
              visibility = excluded.visibility,
              fields_json = excluded.fields_json,
              active = excluded.active,
              updated_at = excluded.updated_at`,
          args: [
            definition.projectionId,
            definition.ownerPluginId,
            definition.schemaVersion,
            definition.configHash,
            definition.visibility,
            JSON.stringify(definition.fields),
            definition.active ? 1 : 0,
            definition.updatedAt,
          ],
        },
      ],
      ["projection"],
    );
  }

  async unregisterProjectionDefinition(projectionId: string): Promise<void> {
    await this.commit(
      [
        {
          sql: "UPDATE index_projections SET active = 0, updated_at = ? WHERE projection_id = ?",
          args: [Date.now(), projectionId],
        },
        {
          sql: "DELETE FROM index_projection_edges WHERE projection_id = ?",
          args: [projectionId],
        },
        {
          sql: "DELETE FROM index_projection_values WHERE projection_id = ?",
          args: [projectionId],
        },
        {
          sql: "DELETE FROM index_projection_rows WHERE projection_id = ?",
          args: [projectionId],
        },
        {
          sql: "DELETE FROM index_projection_sources WHERE projection_id = ?",
          args: [projectionId],
        },
      ],
      ["projection"],
    );
  }

  async replaceProjectionSource(
    input: ReplaceProjectionSourceInput,
  ): Promise<void> {
    assertProjectionWriteAccess(input.projectionId, input.writerPluginId);
    if (input.rows.length > MAX_PROJECTION_ROWS_PER_SOURCE)
      throw new Error(
        `Projection ${input.projectionId} exceeded ${MAX_PROJECTION_ROWS_PER_SOURCE} rows per source.`,
      );
    const definition = await this.getProjectionDefinition(input.projectionId);
    if (!definition?.active) throw new Error("Projection is not registered.");
    const statements = this.deleteProjectionSourceStatements(
      input.projectionId,
      input.sourcePath,
    );
    statements.push({
      sql: `INSERT INTO index_projection_sources
            (projection_id, source_path, source_hash, schema_version, config_hash, status, error, indexed_at)
            VALUES (?, ?, ?, ?, ?, 'ready', NULL, ?)`,
      args: [
        input.projectionId,
        input.sourcePath,
        input.sourceHash,
        definition.schemaVersion,
        definition.configHash,
        Date.now(),
      ],
    });
    for (const row of input.rows) {
      statements.push({
        sql: `INSERT INTO index_projection_rows
              (projection_id, row_id, source_path, kind, ordinal, data_json)
              VALUES (?, ?, ?, ?, ?, ?)`,
        args: [
          input.projectionId,
          row.id,
          input.sourcePath,
          row.kind,
          row.ordinal ?? 0,
          JSON.stringify(row.data),
        ],
      });
      for (const value of indexedValuesForRow(
        input.projectionId,
        row,
        definition.fields,
      )) {
        statements.push({
          sql: `INSERT INTO index_projection_values
                (projection_id, row_id, field, ordinal, value_type, text_value, number_value, boolean_value, date_value, datetime_value)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
          args: [
            value.projectionId,
            value.rowId,
            value.field,
            value.ordinal,
            value.valueType,
            value.textValue ?? null,
            value.numberValue ?? null,
            value.booleanValue == null ? null : value.booleanValue ? 1 : 0,
            value.dateValue ?? null,
            value.datetimeValue ?? null,
          ],
        });
      }
    }
    for (const edge of input.edges ?? [])
      statements.push({
        sql: `INSERT INTO index_projection_edges
            (projection_id, source_row_id, relation, target_projection_id, target_row_id, target_path, target_text, ordinal, data_json)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        args: [
          input.projectionId,
          edge.sourceRowId,
          edge.relation,
          edge.targetProjectionId ?? null,
          edge.targetRowId ?? null,
          edge.targetPath ?? null,
          edge.targetText ?? null,
          edge.ordinal,
          JSON.stringify(edge.data ?? null),
        ],
      });
    await this.commit(statements, ["projection"], [input.sourcePath]);
  }

  async markProjectionSourceError(
    input: MarkProjectionSourceErrorInput,
  ): Promise<void> {
    assertProjectionWriteAccess(input.projectionId, input.writerPluginId);
    const definition = await this.getProjectionDefinition(input.projectionId);
    if (!definition?.active) throw new Error("Projection is not registered.");
    await this.commit(
      [
        ...this.deleteProjectionSourceStatements(
          input.projectionId,
          input.sourcePath,
        ),
        {
          sql: `INSERT INTO index_projection_sources
              (projection_id, source_path, source_hash, schema_version, config_hash, status, error, indexed_at)
              VALUES (?, ?, ?, ?, ?, 'error', ?, ?)`,
          args: [
            input.projectionId,
            input.sourcePath,
            input.sourceHash,
            definition.schemaVersion,
            definition.configHash,
            input.error,
            Date.now(),
          ],
        },
      ],
      ["projection"],
      [input.sourcePath],
    );
  }

  async deleteProjectionSource(
    projectionId: string,
    sourcePath: string,
    writerPluginId?: string,
  ): Promise<void> {
    assertProjectionWriteAccess(projectionId, writerPluginId);
    await this.commit(
      this.deleteProjectionSourceStatements(projectionId, sourcePath),
      ["projection"],
      [sourcePath],
    );
  }

  async queryProjection<T = Record<string, unknown>>(
    projectionId: string,
    query: IndexQuery = {},
    readerPluginId?: string,
  ): Promise<IndexQueryResult<T>> {
    const definition = await this.getProjectionDefinition(projectionId);
    assertProjectionReadAccess(definition, readerPluginId);
    const compiled = compileProjectionQuerySql(
      projectionId,
      projectionQueryCandidates(query),
      definition!,
    );
    const [rows, sources] = await Promise.all([
      this.requireConnection().all<Record<string, unknown>>(
        compiled.sql,
        ...compiled.args,
      ),
      this.getProjectionSources(projectionId),
    ]);
    const records = rows.map(
      (row) =>
        ({
          projectionId,
          rowId: String(row.row_id),
          sourcePath: String(row.source_path),
          kind: String(row.kind),
          ordinal: Number(row.ordinal),
          data: parseJson<Record<string, unknown>>(row.data_json, {}),
        }) satisfies IndexProjectionRowRecord,
    );
    return evaluateProjectionQuery<T>(
      records,
      query,
      this.currentRevision,
      projectionIndexStatus(sources),
    );
  }

  async getProjectionRow<T = Record<string, unknown>>(
    projectionId: string,
    rowId: string,
    readerPluginId?: string,
  ): Promise<T | null> {
    const definition = await this.getProjectionDefinition(projectionId);
    assertProjectionReadAccess(definition, readerPluginId);
    const row = await this.requireConnection().get<{ data_json: string }>(
      `SELECT r.data_json FROM index_projection_rows r
       INNER JOIN index_projection_sources s ON s.projection_id = r.projection_id AND s.source_path = r.source_path
       LEFT JOIN files f ON f.path = r.source_path
       WHERE r.projection_id = ?
         AND (r.row_id = ? OR r.source_path = ? OR json_extract(r.data_json, '$.documentId') = ? OR json_extract(r.data_json, '$.id') = ?)
         AND s.status = 'ready' AND s.schema_version = ? AND s.config_hash = ?
         AND (f.path IS NULL OR (f.hash = s.source_hash AND f.deleted = 0))
       LIMIT 1`,
      projectionId,
      rowId,
      rowId,
      rowId,
      rowId,
      definition!.schemaVersion,
      definition!.configHash,
    );
    return row ? parseJson<T | null>(row.data_json, null) : null;
  }

  async queryRelated<T = Record<string, unknown>>(
    query: IndexRelatedQuery,
    readerPluginId?: string,
  ): Promise<IndexQueryResult<T>> {
    const definition = await this.getProjectionDefinition(query.projectionId);
    assertProjectionReadAccess(definition, readerPluginId);
    const edges = await this.requireConnection().all<Record<string, unknown>>(
      query.direction === "in"
        ? `SELECT * FROM index_projection_edges
           WHERE projection_id = ? AND relation = ?
             AND (target_row_id = ? OR target_path = (
               SELECT source_path FROM index_projection_rows
               WHERE projection_id = ? AND row_id = ?
             )) ORDER BY ordinal`
        : `SELECT * FROM index_projection_edges WHERE projection_id = ? AND relation = ? AND source_row_id = ? ORDER BY ordinal`,
      query.projectionId,
      query.relation,
      query.rowId,
      ...(query.direction === "in" ? [query.projectionId, query.rowId] : []),
    );
    const rows: IndexProjectionRowRecord[] = [];
    for (const edge of edges) {
      const targetId =
        query.direction === "in"
          ? String(edge.source_row_id)
          : edge.target_row_id
            ? String(edge.target_row_id)
            : null;
      const targetProjection =
        query.direction === "in"
          ? query.projectionId
          : String(edge.target_projection_id ?? query.projectionId);
      const data = targetId
        ? await this.getProjectionRow<Record<string, unknown>>(
            targetProjection,
            targetId,
            readerPluginId,
          )
        : edge.target_path
          ? await this.getProjectionRow<Record<string, unknown>>(
              targetProjection,
              String(edge.target_path),
              readerPluginId,
            )
          : null;
      if (!data) continue;
      rows.push({
        projectionId: targetProjection,
        rowId: String(
          (data as { id?: unknown; documentId?: unknown }).id ??
            (data as { documentId?: unknown }).documentId ??
            targetId ??
            edge.target_path,
        ),
        sourcePath: String(
          (data as { documentPath?: unknown }).documentPath ??
            edge.target_path ??
            "",
        ),
        kind: "related",
        ordinal: Number(edge.ordinal),
        data,
      });
    }
    return evaluateProjectionQuery<T>(
      rows,
      { where: query.targetWhere, limit: query.limit },
      this.currentRevision,
      "ready",
    );
  }

  private async commit(
    statements: Array<string | TursoStatementInput>,
    domains: AppDatabaseChangeDomain[],
    paths: string[] = [],
    renamed?: { oldPath: string; newPath: string }[],
  ): Promise<AppDatabaseChangeSet> {
    const committed = this.commitQueue.then(() =>
      this.commitImmediately(statements, domains, paths, renamed),
    );
    this.commitQueue = committed.then(
      () => undefined,
      () => undefined,
    );
    return committed;
  }

  private async commitImmediately(
    statements: Array<string | TursoStatementInput>,
    domains: AppDatabaseChangeDomain[],
    paths: string[] = [],
    renamed?: { oldPath: string; newPath: string }[],
    onCommitted?: () => void,
  ): Promise<AppDatabaseChangeSet> {
    const revision = this.currentRevision + 1;
    const change: AppDatabaseChangeSet = {
      revision,
      domains: [...new Set(domains)],
      paths: [...new Set(paths)],
      renamed: renamed?.map((entry) => ({ ...entry })),
      committedAt: Date.now(),
    };
    await this.requireConnection().batch(
      [
        ...statements,
        {
          sql: `INSERT INTO app_changes
              (revision, domains_json, paths_json, renamed_json, committed_at)
              VALUES (?, ?, ?, ?, ?)`,
          args: [
            revision,
            JSON.stringify(change.domains),
            JSON.stringify(change.paths),
            JSON.stringify(change.renamed ?? null),
            change.committedAt,
          ],
        },
        {
          sql: `INSERT INTO app_revision (id, revision) VALUES (1, ?)
              ON CONFLICT(id) DO UPDATE SET revision = excluded.revision`,
          args: [revision],
        },
      ],
      "immediate",
    );
    this.currentRevision = revision;
    onCommitted?.();
    for (const listener of this.changeListeners) listener(clone(change));
    return change;
  }

  private async updateNotification(
    id: string,
    update: (
      record: AppDatabaseNotificationRecord,
    ) => AppDatabaseNotificationRecord,
  ): Promise<void> {
    const row = await this.requireConnection().get<{ data_json: string }>(
      "SELECT data_json FROM notifications WHERE id = ?",
      id,
    );
    if (!row) return;
    const record = update(
      parseJson<AppDatabaseNotificationRecord>(
        row.data_json,
        {} as AppDatabaseNotificationRecord,
      ),
    );
    await this.commit(
      [
        {
          sql: "UPDATE notifications SET data_json = ? WHERE id = ?",
          args: [JSON.stringify(record), id],
        },
      ],
      ["notification"],
    );
  }

  private fileRecordFromRow(row: Record<string, unknown>) {
    return {
      path: String(row.path),
      normalizedPath: String(row.normalized_path),
      extension: String(row.extension),
      mtime: Number(row.mtime),
      size: Number(row.size),
      hash: String(row.hash),
      parserVersion:
        row.parser_version == null ? undefined : String(row.parser_version),
      indexed: Boolean(row.indexed),
      deleted: Boolean(row.deleted),
    };
  }

  private async readIndexedFiles(
    paths: string[],
    include?: AppDatabaseIndexedMetadataDomain[],
  ): Promise<AppDatabaseIndexedMetadataRow[]> {
    if (!paths.length) return [];
    const selected = include === undefined ? null : new Set(include);
    const includes = (domain: AppDatabaseIndexedMetadataDomain) =>
      selected === null || selected.has(domain);
    const chunks = chunkPaths([...new Set(paths)]);
    const connection = this.requireConnection();

    const readFiles = async () => {
      const rows: Record<string, unknown>[] = [];
      for (const chunk of chunks) {
        rows.push(
          ...(await connection.all<Record<string, unknown>>(
            `SELECT f.path, f.normalized_path, f.extension, f.mtime, f.size,
                    f.hash, f.indexed, f.deleted, m.parser_version
             FROM files f
             LEFT JOIN metadata m ON m.path = f.path
             WHERE f.indexed = 1 AND f.deleted = 0
               AND f.path IN (${chunk.map(() => "?").join(", ")})
             ORDER BY f.path`,
            ...chunk,
          )),
        );
      }
      return rows;
    };

    const readMetadata = async () => {
      if (!includes("metadata")) return [];
      const rows: Record<string, unknown>[] = [];
      for (const chunk of chunks) {
        rows.push(
          ...(await connection.all<Record<string, unknown>>(
            `SELECT path, hash, parser_version, data_json
             FROM metadata
             WHERE path IN (${chunk.map(() => "?").join(", ")})`,
            ...chunk,
          )),
        );
      }
      return rows;
    };

    const readDomain = async (
      domain: Exclude<AppDatabaseIndexedMetadataDomain, "metadata">,
      table: string,
      pathColumn: string,
    ) => {
      if (!includes(domain)) return [];
      const rows: Array<{ path: string; data_json: string }> = [];
      for (const chunk of chunks) {
        rows.push(
          ...(await connection.all<{ path: string; data_json: string }>(
            `SELECT ${pathColumn} AS path, data_json
             FROM ${table}
             WHERE ${pathColumn} IN (${chunk.map(() => "?").join(", ")})
             ORDER BY ${pathColumn}, ordinal`,
            ...chunk,
          )),
        );
      }
      return rows;
    };

    const [fileRows, metadataRows, propertyRows, tagRows, linkRows] =
      await Promise.all([
        readFiles(),
        readMetadata(),
        readDomain("properties", "metadata_properties", "path"),
        readDomain("tags", "metadata_tags", "path"),
        readDomain("links", "metadata_links", "source_path"),
      ]);

    const metadataByPath = new Map(
      metadataRows.map((row) => [
        String(row.path),
        {
          path: String(row.path),
          hash: String(row.hash),
          parserVersion: String(row.parser_version),
          metadata: parseJson(row.data_json, {}),
        },
      ]),
    );
    const collectDomain = <T>(
      rows: Array<{ path: string; data_json: string }>,
    ): Map<string, T[]> => {
      const values = new Map<string, T[]>();
      for (const row of rows) {
        const current = values.get(String(row.path)) ?? [];
        current.push(parseJson<T>(row.data_json, {} as T));
        values.set(String(row.path), current);
      }
      return values;
    };
    const propertiesByPath =
      collectDomain<AppDatabaseIndexedMetadataRow["properties"][number]>(
        propertyRows,
      );
    const tagsByPath =
      collectDomain<AppDatabaseIndexedMetadataRow["tags"][number]>(tagRows);
    const linksByPath = collectDomain<AppDatabaseLinkRecord>(linkRows);
    const filesByPath = new Map(
      fileRows.map((row) => [String(row.path), this.fileRecordFromRow(row)]),
    );

    return paths.flatMap((path) => {
      const file = filesByPath.get(path);
      if (!file) return [];
      return [
        {
          file,
          metadata: includes("metadata")
            ? (metadataByPath.get(path) ?? null)
            : null,
          properties: includes("properties")
            ? (propertiesByPath.get(path) ?? [])
            : [],
          tags: includes("tags") ? (tagsByPath.get(path) ?? []) : [],
          links: includes("links") ? (linksByPath.get(path) ?? []) : [],
        } satisfies AppDatabaseIndexedMetadataRow,
      ];
    });
  }

  private async readIndexedFile(
    path: string,
    legacyOnly: boolean,
  ): Promise<AppDatabaseIndexedMetadataRow | undefined> {
    const file = await this.requireConnection().get<Record<string, unknown>>(
      `SELECT path, normalized_path, extension, mtime, size, hash, indexed, deleted
       FROM files WHERE path = ? AND indexed = 1 AND deleted = 0`,
      path,
    );
    if (!file) return undefined;
    const [metadata, properties, tags, links] = await Promise.all([
      this.requireConnection().get<Record<string, unknown>>(
        "SELECT path, hash, parser_version, data_json FROM metadata WHERE path = ?",
        path,
      ),
      this.requireConnection().all<{ data_json: string }>(
        legacyOnly
          ? "SELECT data_json FROM properties WHERE path = ? ORDER BY ordinal"
          : "SELECT data_json FROM metadata_properties WHERE path = ? ORDER BY ordinal",
        path,
      ),
      this.requireConnection().all<{ data_json: string }>(
        legacyOnly
          ? "SELECT data_json FROM tags WHERE path = ? ORDER BY ordinal"
          : "SELECT data_json FROM metadata_tags WHERE path = ? ORDER BY ordinal",
        path,
      ),
      this.requireConnection().all<{ data_json: string }>(
        legacyOnly
          ? "SELECT data_json FROM links WHERE source_path = ? ORDER BY ordinal"
          : "SELECT data_json FROM metadata_links WHERE source_path = ? ORDER BY ordinal",
        path,
      ),
    ]);
    return {
      file: this.fileRecordFromRow(file),
      metadata: metadata
        ? {
            path: String(metadata.path),
            hash: String(metadata.hash),
            parserVersion: String(metadata.parser_version),
            metadata: parseJson(metadata.data_json, {}),
          }
        : null,
      properties: properties.map((row) =>
        parseJson(
          row.data_json,
          {} as AppDatabaseIndexedMetadataRow["properties"][number],
        ),
      ),
      tags: tags.map((row) =>
        parseJson(
          row.data_json,
          {} as AppDatabaseIndexedMetadataRow["tags"][number],
        ),
      ),
      links: links.map((row) =>
        parseJson(row.data_json, {} as AppDatabaseLinkRecord),
      ),
    };
  }

  private compileMetadataPathQuery(
    query: AppDatabaseIndexedMetadataQuery,
    after: string | undefined,
    limit: number,
  ): { sql: string; args: unknown[] } {
    const args: unknown[] = [];
    const conditions = ["f.indexed = 1", "f.deleted = 0"];
    if (after) {
      conditions.push("f.path > ?");
      args.push(after);
    }
    if (query.extensions?.length) {
      conditions.push(
        `f.extension IN (${query.extensions.map(() => "?").join(", ")})`,
      );
      args.push(...query.extensions.map(normalizeExtension));
    }
    if (query.pathPrefixes?.length) {
      const prefixClauses = query.pathPrefixes.map(
        () => "(f.path = ? OR f.path LIKE ? ESCAPE '\\')",
      );
      conditions.push(balancedSqlDisjunction(prefixClauses));
      for (const raw of query.pathPrefixes) {
        const prefix = raw.replace(/^\/+|\/+$/g, "");
        args.push(
          prefix,
          `${prefix.replaceAll("%", "\\%").replaceAll("_", "\\_")}/%`,
        );
      }
    }
    if (query.excludeHiddenPaths) {
      conditions.push("f.path NOT GLOB '.*' AND f.path NOT GLOB '*/.*'");
    }
    for (const tag of query.requiredTags ?? []) {
      conditions.push(
        "EXISTS (SELECT 1 FROM metadata_tag_ancestors ta WHERE ta.path = f.path AND ta.ancestor = ?)",
      );
      args.push(normalizeTag(tag));
    }
    for (const target of query.resolvedTargetPaths ?? []) {
      conditions.push(
        "EXISTS (SELECT 1 FROM metadata_links ml WHERE ml.source_path = f.path AND ml.resolved_target_path = ?)",
      );
      args.push(target);
    }
    for (const filter of query.propertyFilters ?? []) {
      conditions.push(this.compilePropertyFilter(filter, args));
    }
    const order = (query.sort ?? []).map((sort) => {
      const direction = sort.direction === "DESC" ? "DESC" : "ASC";
      if (sort.field.kind === "file")
        return `f.${sort.field.field === "path" ? "path" : sort.field.field} ${direction}`;
      args.push(sort.field.name.toLowerCase());
      return `(SELECT COALESCE(pv.number_value, pv.boolean_value, pv.date_value, pv.text_value)
               FROM metadata_property_values pv
               WHERE pv.path = f.path AND pv.property_name = ?
               ORDER BY pv.property_ordinal, pv.value_ordinal LIMIT 1) ${direction}`;
    });
    order.push("f.path ASC");
    args.push(limit);
    return {
      sql: `SELECT f.path FROM files f WHERE ${conditions.join(" AND ")}
            ORDER BY ${order.join(", ")} LIMIT ?`,
      args,
    };
  }

  private compilePropertyFilter(
    filter: AppDatabaseIndexedMetadataPropertyFilter,
    args: unknown[],
  ): string {
    const name = normalizePropertyPath(filter.name);
    if (filter.op === "exists" || filter.op === "not-exists") {
      args.push(name, name);
      return `${filter.op === "not-exists" ? "NOT " : ""}(
        EXISTS (
          SELECT 1 FROM metadata_properties mp
          WHERE mp.path = f.path AND mp.normalized_name = ?
        ) OR EXISTS (
          SELECT 1 FROM metadata_property_values pv
          WHERE pv.path = f.path AND pv.normalized_property_path = ?
        )
      )`;
    }
    const operator =
      filter.op === "=" ? "=" : filter.op === "!=" ? "!=" : filter.op;
    const value = filter.value;
    const column =
      typeof value === "number"
        ? "number_value"
        : typeof value === "boolean"
          ? "boolean_value"
          : typeof value === "string" && isDateScalar(value)
            ? "date_value"
            : "text_value";
    args.push(
      name,
      name,
      typeof value === "boolean" ? (value ? 1 : 0) : (value ?? null),
    );
    return `EXISTS (
      SELECT 1 FROM metadata_property_values pv
      WHERE pv.path = f.path
        AND (pv.property_name = ? OR pv.normalized_property_path = ?)
        AND pv.${column} ${operator} ?
    )`;
  }

  private compilePathPrefixes(
    column: string,
    prefixes: string[] | undefined,
    args: unknown[],
  ): string {
    if (!prefixes?.length) return "";
    const clauses = prefixes.map(
      () => `(${column} = ? OR ${column} LIKE ? ESCAPE '\\')`,
    );
    for (const raw of prefixes) {
      const prefix = raw.replace(/^\/+|\/+$/g, "");
      args.push(
        prefix,
        `${prefix.replaceAll("%", "\\%").replaceAll("_", "\\_")}/%`,
      );
    }
    return `AND ${balancedSqlDisjunction(clauses)}`;
  }

  private deletePathStatements(path: string): TursoStatementInput[] {
    return [
      ...this.deleteMetadataPathStatements(path),
      { sql: "DELETE FROM search_chunks WHERE path = ?", args: [path] },
      { sql: "DELETE FROM search_docs WHERE path = ?", args: [path] },
      { sql: "DELETE FROM task_records WHERE document_path = ?", args: [path] },
      {
        sql: "DELETE FROM index_projection_edges WHERE source_row_id IN (SELECT row_id FROM index_projection_rows WHERE source_path = ?)",
        args: [path],
      },
      {
        sql: "DELETE FROM index_projection_values WHERE row_id IN (SELECT row_id FROM index_projection_rows WHERE source_path = ?)",
        args: [path],
      },
      {
        sql: "DELETE FROM index_projection_rows WHERE source_path = ?",
        args: [path],
      },
      {
        sql: "DELETE FROM index_projection_sources WHERE source_path = ?",
        args: [path],
      },
    ];
  }

  private deleteMetadataPathStatements(path: string): TursoStatementInput[] {
    return [
      {
        sql: "DELETE FROM app_meta WHERE key = ?",
        args: [notebookStateMetaKey(path)],
      },
      { sql: "DELETE FROM metadata WHERE path = ?", args: [path] },
      { sql: "DELETE FROM links WHERE source_path = ?", args: [path] },
      { sql: "DELETE FROM tags WHERE path = ?", args: [path] },
      { sql: "DELETE FROM properties WHERE path = ?", args: [path] },
      { sql: "DELETE FROM metadata_links WHERE source_path = ?", args: [path] },
      { sql: "DELETE FROM metadata_tags WHERE path = ?", args: [path] },
      {
        sql: "DELETE FROM metadata_tag_ancestors WHERE path = ?",
        args: [path],
      },
      { sql: "DELETE FROM metadata_properties WHERE path = ?", args: [path] },
      {
        sql: "DELETE FROM metadata_property_values WHERE path = ?",
        args: [path],
      },
    ];
  }

  private searchDocumentStatements(
    document: SearchDocumentRecord,
    previousPath?: string,
  ): TursoStatementInput[] {
    const statements: TursoStatementInput[] = [];
    if (previousPath) {
      statements.push(
        {
          sql: "DELETE FROM search_chunks WHERE path = ?",
          args: [previousPath],
        },
        { sql: "DELETE FROM search_docs WHERE path = ?", args: [previousPath] },
      );
    }
    statements.push(
      {
        sql: "DELETE FROM search_chunks WHERE path = ?",
        args: [document.path],
      },
      {
        sql: `INSERT INTO search_docs
              (path, source_provider_id, metadata_hash, provider_version,
               projection_signature, source_mtime, source_size,
               name, extension, checksum, content, tags,
               metadata_text, data_json)
              VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
              ON CONFLICT(path) DO UPDATE SET
                source_provider_id = excluded.source_provider_id,
                metadata_hash = excluded.metadata_hash,
                provider_version = excluded.provider_version,
                projection_signature = excluded.projection_signature,
                source_mtime = excluded.source_mtime,
                source_size = excluded.source_size,
                name = excluded.name,
                extension = excluded.extension,
                checksum = excluded.checksum,
                content = excluded.content,
                tags = excluded.tags,
                metadata_text = excluded.metadata_text,
                data_json = excluded.data_json`,
        args: [
          document.path,
          document.sourceProviderId ?? null,
          document.sourceMetadata?.metadataHash ?? null,
          document.sourceMetadata?.providerVersion ?? null,
          document.sourceMetadata?.projectionSignature ?? null,
          document.sourceMetadata?.sourceMtime ?? null,
          document.sourceMetadata?.sourceSize ?? null,
          document.name,
          document.extension,
          document.checksum,
          document.content,
          document.tags.join(" "),
          document.metadataText ?? "",
          JSON.stringify(document),
        ],
      },
    );
    for (const chunk of document.chunks ?? []) {
      const vector = chunk.embedding?.vector;
      statements.push({
        sql:
          this.nativeVectorSearch && vector?.length
            ? `INSERT INTO search_chunks (id, path, text, embedding_json, embedding)
             VALUES (?, ?, ?, ?, vector32(?))`
            : `INSERT INTO search_chunks (id, path, text, embedding_json, embedding)
             VALUES (?, ?, ?, ?, NULL)`,
        args:
          this.nativeVectorSearch && vector?.length
            ? [
                chunk.id,
                document.path,
                chunk.text,
                JSON.stringify(chunk.embedding ?? null),
                JSON.stringify(vector),
              ]
            : [
                chunk.id,
                document.path,
                chunk.text,
                JSON.stringify(chunk.embedding ?? null),
              ],
      });
    }
    return statements;
  }

  private async prepareSearchDocument(
    document: SearchDocumentRecord,
  ): Promise<SearchDocumentRecord> {
    const normalized = normalizeSearchDocument(document);
    const provider = this.searchEmbeddingProvider;
    if (!provider || !normalized.chunks?.length) return clone(normalized);
    try {
      if (!(await provider.ready())) {
        return {
          ...clone(normalized),
          chunks: normalized.chunks.map((chunk) => ({
            ...clone(chunk),
            embedding: {
              ...clone(chunk.embedding),
              status: "pending",
              modelId: provider.config.modelId ?? "lapis/token-hash-v0",
              modelVersion: provider.config.modelVersion,
              dimensions: provider.config.dimensions,
            },
          })),
        };
      }
      const embeddings = new Map(
        (await provider.embedDocument(normalized)).map((entry) => [
          entry.chunkId,
          entry,
        ]),
      );
      const timestamp = Date.now();
      return {
        ...clone(normalized),
        chunks: normalized.chunks.map((chunk) => {
          const embedding = embeddings.get(chunk.id);
          return embedding
            ? {
                ...clone(chunk),
                embedding: {
                  status: "ready",
                  modelId: provider.config.modelId ?? "lapis/token-hash-v0",
                  modelVersion: provider.config.modelVersion,
                  dimensions: embedding.vector.length,
                  vector: [...embedding.vector],
                  fingerprint: embedding.fingerprint,
                  dirty: false,
                  updatedAt: timestamp,
                },
              }
            : clone(chunk);
        }),
      };
    } catch (error) {
      return embeddingErrorState(normalized, provider, error);
    }
  }

  private async evaluateSearchDocumentsForPaths(
    query: string,
    options: AppDatabaseSearchOptions,
    candidatePaths: Iterable<string>,
  ): Promise<AppDatabaseSearchResult[]> {
    const paths = [...new Set(candidatePaths)];
    if (!paths.length) return [];
    const rows: Array<{ path: string; data_json: string }> = [];
    for (const chunk of chunkPaths(paths)) {
      rows.push(
        ...(await this.requireConnection().all<{
          path: string;
          data_json: string;
        }>(
          `SELECT path, data_json FROM search_docs
           WHERE path IN (${chunk.map(() => "?").join(", ")})`,
          ...chunk,
        )),
      );
    }
    const propertyNames = searchPropertyNames(query);
    const allowedProviders = options.sourceProviderIds?.length
      ? new Set(options.sourceProviderIds)
      : null;
    const propertyRows = await this.readIndexedFiles(paths, ["properties"]);
    const propertiesByPath = new Map(
      propertyRows.map((row) => [row.file.path, row.properties]),
    );
    const sourceDocuments = rows.map((row) => {
      const document = parseJson<SearchDocumentRecord>(
        row.data_json,
        {} as SearchDocumentRecord,
      );
      return {
        document,
        properties: searchDocumentProperties(
          document,
          propertiesByPath.get(document.path) ?? [],
        ),
      };
    });
    const filtered = sourceDocuments.filter(
      ({ document, properties }) =>
        pathWithinPrefix(document.path, options.pathPrefix) &&
        (!allowedProviders ||
          (document.sourceProviderId != null &&
            allowedProviders.has(document.sourceProviderId))) &&
        hasSearchPropertyNames(properties, propertyNames),
    );
    const requestedMode = options.mode ?? "auto";
    const queryVector =
      requestedMode === "lexical" || !this.searchEmbeddingProvider
        ? null
        : await this.safeEmbedQuery(query);
    const vectorScores = new Map(
      filtered.map(({ document }) => [
        document.path,
        queryVector
          ? scoreVectorDocument(document, queryVector)
          : { score: 0, matchedChunkIds: [] },
      ]),
    );
    const lexicalScores = new Map(
      filtered.map(({ document, properties }) => [
        document.path,
        scoreSearchDocument(document, query, properties, options),
      ]),
    );
    const vectorCandidateCount = [...vectorScores.values()].filter(
      (entry) => entry.score >= MIN_VECTOR_SEARCH_SCORE,
    ).length;
    const appliedMode = queryVector
      ? requestedMode === "vector"
        ? "vector"
        : vectorCandidateCount
          ? "hybrid"
          : "lexical"
      : "lexical";
    const lexicalRanks = rankSearchScores(
      [...lexicalScores].map(([path, score]) => ({ path, score })),
    );
    const vectorRanks = rankSearchScores(
      [...vectorScores].map(([path, entry]) => ({ path, score: entry.score })),
      (score) => score >= MIN_VECTOR_SEARCH_SCORE,
    );
    const results = filtered
      .map(({ document }) =>
        buildSearchResult(document, query, options, {
          backendKind: this.kind,
          appliedMode,
          lexicalScore: lexicalScores.get(document.path) ?? 0,
          vectorScore: vectorScores.get(document.path)?.score ?? 0,
          lexicalCandidateCount: filtered.length,
          vectorCandidateCount,
          providerConfig: this.searchEmbeddingProviderConfig,
          preferredChunkIds:
            vectorScores.get(document.path)?.matchedChunkIds ?? [],
          lexicalRank: lexicalRanks.get(document.path),
          vectorRank: vectorRanks.get(document.path),
        }),
      )
      .filter((result) => {
        const lexicalHit = (result.scoreBreakdown.lexical ?? 0) > 0;
        const vectorHit =
          (result.scoreBreakdown.vector ?? 0) >= MIN_VECTOR_SEARCH_SCORE;
        return appliedMode === "vector"
          ? vectorHit && result.snippets.length > 0
          : appliedMode === "hybrid"
            ? (lexicalHit || vectorHit) && result.snippets.length > 0
            : lexicalHit && result.snippets.length > 0;
      })
      .sort(compareSearchResults);
    const diagnostics = resolveSearchQueryEnhancementDiagnostics(
      results,
      options,
    );
    return results.slice(0, options.limit ?? 100).map((result) => {
      const copy = clone(result);
      if (copy.diagnostics && diagnostics)
        copy.diagnostics.queryEnhancement = clone(diagnostics);
      return copy;
    });
  }

  private async safeEmbedQuery(query: string): Promise<number[] | null> {
    try {
      return this.searchEmbeddingProvider
        ? await this.searchEmbeddingProvider.embedQuery(query)
        : null;
    } catch {
      return null;
    }
  }

  private deleteProjectionSourceStatements(
    projectionId: string,
    sourcePath: string,
  ): TursoStatementInput[] {
    return [
      {
        sql: "DELETE FROM index_projection_edges WHERE projection_id = ? AND source_row_id IN (SELECT row_id FROM index_projection_rows WHERE projection_id = ? AND source_path = ?)",
        args: [projectionId, projectionId, sourcePath],
      },
      {
        sql: "DELETE FROM index_projection_values WHERE projection_id = ? AND row_id IN (SELECT row_id FROM index_projection_rows WHERE projection_id = ? AND source_path = ?)",
        args: [projectionId, projectionId, sourcePath],
      },
      {
        sql: "DELETE FROM index_projection_rows WHERE projection_id = ? AND source_path = ?",
        args: [projectionId, sourcePath],
      },
      {
        sql: "DELETE FROM index_projection_sources WHERE projection_id = ? AND source_path = ?",
        args: [projectionId, sourcePath],
      },
    ];
  }

  private async getProjectionDefinition(
    projectionId: string,
  ): Promise<IndexProjectionDefinitionRecord | undefined> {
    const row = await this.requireConnection().get<Record<string, unknown>>(
      "SELECT * FROM index_projections WHERE projection_id = ?",
      projectionId,
    );
    return row
      ? {
          projectionId: String(row.projection_id),
          ownerPluginId: String(row.owner_plugin_id),
          schemaVersion: Number(row.schema_version),
          configHash: String(row.config_hash),
          visibility: String(
            row.visibility,
          ) as IndexProjectionDefinitionRecord["visibility"],
          fields: parseJson(row.fields_json, {}),
          active: Boolean(row.active),
          updatedAt: Number(row.updated_at),
        }
      : undefined;
  }

  private async getProjectionSources(
    projectionId: string,
  ): Promise<IndexProjectionSourceRecord[]> {
    const rows = await this.requireConnection().all<Record<string, unknown>>(
      "SELECT * FROM index_projection_sources WHERE projection_id = ?",
      projectionId,
    );
    return rows.map((row) => ({
      projectionId: String(row.projection_id),
      sourcePath: String(row.source_path),
      sourceHash: String(row.source_hash),
      schemaVersion: Number(row.schema_version),
      configHash: String(row.config_hash),
      status: String(row.status) as IndexProjectionSourceRecord["status"],
      error: row.error == null ? null : String(row.error),
      indexedAt: Number(row.indexed_at),
    }));
  }

  private async ensureTasksProjectionDefinition(): Promise<void> {
    if (
      (await this.getProjectionDefinition(PUBLIC_TASKS_PROJECTION_ID))?.active
    )
      return;
    await this.registerProjectionDefinition({
      projectionId: PUBLIC_TASKS_PROJECTION_ID,
      ownerPluginId: "tasks",
      schemaVersion: TASK_PROJECTION_VERSION,
      configHash: "",
      visibility: "public",
      fields: TASK_PROJECTION_FIELDS,
      active: true,
      updatedAt: Date.now(),
    });
  }

  private async backfillNormalizedMetadata(): Promise<void> {
    const connection = this.requireConnection();
    const historyBefore = Number(
      (
        await connection.get<{ count: number }>(
          "SELECT count(*) AS count FROM history_revisions",
        )
      )?.count ?? 0,
    );
    const paths = await connection.all<{ path: string }>(
      "SELECT path FROM files WHERE indexed = 1 AND deleted = 0 ORDER BY path",
    );
    for (const { path } of paths) {
      const record = await this.readIndexedFile(path, true);
      if (record?.metadata)
        await connection.batch(
          indexedFileStatements({ ...record, metadata: record.metadata }),
          "immediate",
        );
    }
    const historyFiles = await connection.all<{
      file_id: string;
      data_json: string;
    }>("SELECT file_id, data_json FROM history_files");
    const historyPathStatements = historyFiles.flatMap((row) => {
      const file = parseJson<AppDatabaseFileHistoryFile>(row.data_json, {
        fileId: row.file_id,
        currentPath: "",
        deleted: false,
      });
      return file.currentPath
        ? [
            {
              sql: `INSERT INTO history_file_paths (path, file_id) VALUES (?, ?)
              ON CONFLICT(path) DO UPDATE SET file_id = excluded.file_id`,
              args: [file.currentPath, row.file_id],
            },
          ]
        : [];
    });
    if (historyPathStatements.length) {
      await connection.batch(historyPathStatements, "immediate");
    }
    const historyAfter = Number(
      (
        await connection.get<{ count: number }>(
          "SELECT count(*) AS count FROM history_revisions",
        )
      )?.count ?? 0,
    );
    if (historyBefore !== historyAfter)
      throw new Error("Turso metadata migration changed History revisions");
  }

  private async captureMigrationInvariants(): Promise<Record<string, number>> {
    const connection = this.requireConnection();
    const tables = [
      "app_state",
      "files",
      "metadata",
      "links",
      "tags",
      "properties",
      "search_docs",
      "search_chunks",
      "history_files",
      "history_revisions",
      "notifications",
      "tasks",
      "task_records",
      "index_projections",
      "index_projection_sources",
      "index_projection_rows",
      "index_projection_values",
      "index_projection_edges",
    ];
    const counts: Record<string, number> = {};
    for (const table of tables) {
      counts[table] = Number(
        (
          await connection.get<{ count: number }>(
            `SELECT count(*) AS count FROM ${table}`,
          )
        )?.count ?? 0,
      );
    }
    return counts;
  }

  private async validateMigration(
    expectedCounts: Record<string, number>,
  ): Promise<void> {
    const connection = this.requireConnection();
    const actualCounts = await this.captureMigrationInvariants();
    for (const [table, expected] of Object.entries(expectedCounts)) {
      if (actualCounts[table] !== expected) {
        throw new Error(
          `Turso migration changed ${table} row count (${expected} -> ${actualCounts[table]})`,
        );
      }
    }

    const jsonColumns = [
      ["app_state", "state_json"],
      ["app_meta", "value_json"],
      ["metadata", "data_json"],
      ["links", "data_json"],
      ["tags", "data_json"],
      ["properties", "data_json"],
      ["task_records", "data_json"],
      ["index_projections", "fields_json"],
      ["index_projection_rows", "data_json"],
      ["index_projection_edges", "data_json"],
      ["history_files", "data_json"],
      ["history_revisions", "data_json"],
      ["notifications", "data_json"],
      ["search_docs", "data_json"],
      ["search_chunks", "embedding_json"],
    ] as const;
    for (const [table, column] of jsonColumns) {
      const invalid = Number(
        (
          await connection.get<{ count: number }>(
            `SELECT count(*) AS count FROM ${table} WHERE ${column} IS NOT NULL AND json_valid(${column}) = 0`,
          )
        )?.count ?? 0,
      );
      if (invalid > 0) {
        throw new Error(
          `Turso migration validation found ${invalid} invalid ${table}.${column} rows`,
        );
      }
    }

    for (const [source, normalized] of [
      ["links", "metadata_links"],
      ["tags", "metadata_tags"],
      ["properties", "metadata_properties"],
    ] as const) {
      const sourceCount = actualCounts[source] ?? 0;
      const normalizedCount = Number(
        (
          await connection.get<{ count: number }>(
            `SELECT count(*) AS count FROM ${normalized}`,
          )
        )?.count ?? 0,
      );
      if (normalizedCount !== sourceCount) {
        throw new Error(
          `Turso migration normalized ${normalizedCount}/${sourceCount} ${source} rows`,
        );
      }
    }
  }

  private async ensureMetadataPropertyPathSchema(): Promise<void> {
    const connection = this.requireConnection();
    const columns = await connection.all<{ name: string }>(
      "PRAGMA table_info(metadata_property_values)",
    );
    if (!columns.some((column) => column.name === "normalized_property_path")) {
      await connection.exec(
        "ALTER TABLE metadata_property_values ADD COLUMN normalized_property_path TEXT NOT NULL DEFAULT ''",
      );
    }
    await connection.exec(`
      CREATE INDEX IF NOT EXISTS metadata_property_path_text_idx
        ON metadata_property_values(normalized_property_path, text_value, path);
      CREATE INDEX IF NOT EXISTS metadata_property_path_number_idx
        ON metadata_property_values(normalized_property_path, number_value, path);
      CREATE INDEX IF NOT EXISTS metadata_property_path_boolean_idx
        ON metadata_property_values(normalized_property_path, boolean_value, path);
      CREATE INDEX IF NOT EXISTS metadata_property_path_date_idx
        ON metadata_property_values(normalized_property_path, date_value, path);
    `);
    const searchColumns = await connection.all<{ name: string }>(
      "PRAGMA table_info(search_docs)",
    );
    const searchColumnNames = new Set(
      searchColumns.map((column) => column.name),
    );
    for (const column of [
      "source_provider_id",
      "metadata_hash",
      "provider_version",
      "projection_signature",
      "source_mtime",
      "source_size",
    ]) {
      if (!searchColumnNames.has(column)) {
        await connection.exec(
          `ALTER TABLE search_docs ADD COLUMN ${column} ${column.startsWith("source_") && column !== "source_provider_id" ? "INTEGER" : "TEXT"}`,
        );
      }
    }
    await connection.exec(
      "CREATE INDEX IF NOT EXISTS search_docs_manifest_idx ON search_docs(source_provider_id, path)",
    );
  }

  private requireConnection(): TursoConnection {
    if (!this.connection) throw new Error("Turso app database is not open");
    return this.connection;
  }
}

export function canUseTursoWasmDatabase(): boolean {
  return (
    typeof navigator !== "undefined" &&
    typeof navigator.storage?.getDirectory === "function" &&
    typeof SharedArrayBuffer !== "undefined" &&
    typeof Atomics !== "undefined" &&
    globalThis.crossOriginIsolated === true
  );
}

export async function createTursoWasmConnection(
  path: string,
): Promise<TursoConnection> {
  // The Vite-aware export avoids bundling through the package's prebuilt
  // worker-inline entry, which Rollup's CommonJS resolver can recurse on, and
  // still keeps the dev-server worker/WASM handling owned by the driver.
  const { connect } = (await import(
    "@tursodatabase/database-wasm/vite" as string
  )) as {
    connect(
      databasePath: string,
      options?: { experimental?: string[] },
    ): Promise<TursoConnection>;
  };
  return (await connect(path, {
    experimental: ["index_method"],
  })) as TursoConnection;
}

export class TursoWasmAppDatabaseProvider implements AppDatabaseProvider {
  readonly id = "turso-wasm-local";

  canOpen(context: AppDatabaseOpenContext): boolean {
    return context.runtime !== "test" && canUseTursoWasmDatabase();
  }

  async open(context: AppDatabaseOpenContext): Promise<AppDatabase> {
    if (!(await this.canOpen(context))) {
      throw new Error("Turso WASM requires OPFS and cross-origin isolation");
    }
    const database = new TursoAppDatabase(context.vaultId, {
      kind: "turso-wasm",
      providerId: this.id,
      transport: "wasm-worker",
      role: context.role,
      crossTabCoordination: context.runtime === "web-pwa",
      connectionFactory: () =>
        createTursoWasmConnection(tursoWasmDatabasePath(context.vaultId)),
    });
    await database.open();
    return database;
  }
}
