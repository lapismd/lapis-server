import {
  PROTOCOL_MAJOR,
  type Request,
  type Reply,
  type VaultCommand,
  type AttachClient,
  type ManagedVault,
  type LoginRequest,
  type SessionInfo,
  type MetaTransaction,
  type MetaPage,
  type NotebookState,
  type NotificationRecord,
  type ChangeSet,
  type StoreHistory,
  type CoreIndexedFile,
  type IndexedFile,
  type MetadataPageQuery,
  type ProjectionDefinition,
  type ProjectionSource,
  type ProjectionQuery,
  type ProjectionRelatedQuery,
  type TaskRecord,
  type TaskQuery,
  type MemorySource,
  type MemoryCandidate,
  type MemoryJobClaim,
  type MemoryJobUpdate,
  type MemoryRecallSignal,
  type SearchDocumentRecord,
  type SearchManifestPage,
  type SearchManifestQuery,
  type SearchIndexStats,
  type SearchOptions,
  type SearchEmbeddingProviderConfig,
  type SearchResult,
} from "../bindings/protocol.js";
const command = {
  kind: "write_text",
  path: "note.md",
  text: "hello",
  expected: { kind: "missing" },
} satisfies VaultCommand;
const request: Request = {
  protocol_version: PROTOCOL_MAJOR,
  request_id: "request",
  vault_id: "vault",
  generation: "generation",
  command,
};
// @ts-expect-error Unknown commands cannot enter a typed client.
const unknown: VaultCommand = { kind: "save_anything" };
// @ts-expect-error Conditional writes require an expected version.
const unsafe: VaultCommand = {
  kind: "write_text",
  path: "note.md",
  text: "lost",
};
// @ts-expect-error Discard requires a specific editor.
const discard: VaultCommand = { kind: "discard_editor" };
function result(reply: Reply) {
  if (reply.outcome.status === "error") return reply.outcome.error.code;
  const value = reply.outcome.value;
  if (value.kind === "editor_claims")
    return value.claims.map((claim) => claim.editor_id);
  if (value.kind === "text") return value.snapshot.version;
  return value.kind;
}
void [request, unknown, unsafe, discard, result];

const fileAction: VaultCommand = {
  kind: "files",
  operation: { operation: "rename", path: "old.md", newPath: "new.md" },
};
const escape: VaultCommand = {
  kind: "files",
  operation: {
    // @ts-expect-error Native destinations are never portable file commands.
    operation: "open-path",
    path: "/tmp/other",
  },
};
void [fileAction, escape];

const attach: AttachClient = { protocol_version: PROTOCOL_MAJOR };
const vault: ManagedVault = { vault_id: "vault", name: "Notes" };
const remoteRoot: AttachClient = {
  protocol_version: PROTOCOL_MAJOR,
  // @ts-expect-error Only the operator may configure native roots.
  root: "/tmp/other",
};
void [attach, vault, remoteRoot];

const login: LoginRequest = { username: "owner", password: "disposable" };
const session: SessionInfo = {
  username: "owner",
  csrf_token: "csrf",
  expires_at: 1_700_000_000_000,
};
const claimedIdentity: LoginRequest = {
  username: "owner",
  password: "disposable",
  // @ts-expect-error Session identity is derived from the authenticated cookie.
  session_id: "untrusted",
};
void [login, session, claimedIdentity];

const metadata: MetaTransaction = {
  checks: [{ key: "optional", exists: false }],
  mutations: [{ key: "nullable", value: null }, { key: "deleted" }],
};
const page: MetaPage = { entries: [], nextCursor: "plugin:last" };
const metadataSql: MetaTransaction = {
  mutations: [],
  // @ts-expect-error Metadata transactions never receive SQL.
  sql: "DROP TABLE app_meta",
};
void [metadata, page, metadataSql];

const notebook: NotebookState = {
  sourcePath: "note.md",
  mtime: 12.5,
  updatedAt: 20,
  cells: { first: { state: "complete", outputs: [{ text: "done" }] } },
};
const notice: NotificationRecord = {
  id: "notice",
  message: "Saved",
  severity: "info",
  createdAt: 1,
  updatedAt: 2,
  read: false,
  cleared: false,
};
const change: ChangeSet = {
  revision: 1,
  domains: ["notebook"],
  paths: ["note.md"],
  committedAt: 2,
};
const invalidNotice: NotificationRecord = {
  ...notice,
  // @ts-expect-error Only the existing severity catalogue is admitted.
  severity: "debug",
};
void [notebook, notice, change, invalidNotice];

const history: StoreHistory = {
  path: "note.md",
  eventType: "modify",
  createdAt: 1,
  content: "new",
  contentHash: "hash",
  replaceLatest: true,
  maxRevisions: 10,
};
const invalidHistory: StoreHistory = {
  ...history,
  // @ts-expect-error Native history retains the existing event catalogue.
  eventType: "overwrite",
};
void [history, invalidHistory];

const metadataQuery: MetadataPageQuery = {
  query: {
    propertyFilters: [{ name: "team.members[].active", op: "=", value: true }],
    sort: [{ field: { kind: "file", field: "mtime" }, direction: "DESC" }],
  },
  include: ["properties", "tags"],
  limit: 20,
};
const unknownDomain: MetadataPageQuery = {
  // @ts-expect-error Selective reads use the retained typed domain catalogue.
  include: ["sql"],
};
const corePublication = {} as CoreIndexedFile;
const projectionInCore: CoreIndexedFile = {
  ...corePublication,
  // @ts-expect-error Core file publication is independent of plugin projections.
  task: {},
};
void [metadataQuery, unknownDomain, projectionInCore];

const projection: ProjectionDefinition = {
  projectionId: "books/book",
  ownerPluginId: "books",
  schemaVersion: 2,
  configHash: "config-2",
  visibility: "public",
  fields: { title: { type: "string", indexed: true } },
  active: true,
  updatedAt: 1,
};
const projectionSource: ProjectionSource = {
  projectionId: projection.projectionId,
  sourcePath: "book.md",
  sourceHash: "hash",
  rows: [{ id: "book", kind: "book", data: { title: "Book" } }],
};
const unknownProjection: ProjectionDefinition = {
  ...projection,
  // @ts-expect-error The retained visibility catalogue is explicit.
  visibility: "shared",
};
const projectionSql: ProjectionSource = {
  ...projectionSource,
  // @ts-expect-error The native facade never receives raw SQL.
  sql: "DELETE FROM index_projection_rows",
};
void [projection, projectionSource, unknownProjection, projectionSql];

const projectionQuery: ProjectionQuery = {
  where: {
    op: "and",
    operands: [
      { op: "in", field: "optional", values: [null, "value"] },
      { op: "compare", field: "rank", comparison: "gte", value: 2 },
    ],
  },
  orderBy: [{ field: "rank", direction: "asc", nulls: "last" }],
  select: ["id", "rank"],
  limit: 2,
};
const relatedQuery: ProjectionRelatedQuery = {
  projectionId: "books/book",
  rowId: "a",
  relation: "child",
  direction: "in",
  targetWhere: { op: "not-exists", field: "archived" },
};
const querySql: ProjectionQuery = {
  // @ts-expect-error Native query callers cannot select raw SQL.
  sql: "SELECT * FROM index_projection_rows",
};
const invalidProjectionDirection: ProjectionRelatedQuery = {
  ...relatedQuery,
  // @ts-expect-error The retained edge direction catalogue is explicit.
  direction: "both",
};
void [projectionQuery, relatedQuery, querySql, invalidProjectionDirection];

const task: TaskRecord = {
  documentPath: "task.md",
  documentId: "task",
  kind: "task",
  title: "Task",
  status: "open",
  inbox: false,
  startKind: "anytime",
  checklistTotal: 0,
  checklistCompleted: 0,
  commentCount: 0,
  projectionVersion: 4,
  startDate: null,
  repeat: { rrule: "FREQ=DAILY", anchor: "schedule", missed: "carry" },
  tracking: { type: "count", unit: "pages", target: 20 },
  structure: null,
};
const taskQuery: TaskQuery = { view: "review", today: "2026-10-24", limit: 20 };
const invalidTask: TaskRecord = {
  ...task,
  // @ts-expect-error Task status is the retained closed catalogue.
  status: "paused",
};
const invalidTaskQuery: TaskQuery = {
  // @ts-expect-error Task queries never carry SQL.
  sql: "SELECT * FROM task_records",
};
void [task, taskQuery, invalidTask, invalidTaskQuery];

const memorySource = {
  sourceKey: "conversation",
  sourcePath: "Memory/User",
  sourceHash: "hash",
  status: "ready",
  indexedAt: 1,
} satisfies MemorySource;
const memoryClaim = {
  job: {
    id: "ingest:1",
    kind: "ingest",
    scopeKey: "workspace",
    status: "queued",
    attempts: 0,
    maxAttempts: 2,
    createdAt: 1,
  },
  ownerId: "service",
  now: 2,
  leaseMs: 100,
} satisfies MemoryJobClaim;
const memoryUpdate = {
  jobId: "ingest:1",
  ownerId: "service",
  now: 3,
  patch: { status: "completed", finishedAt: 3 },
} satisfies MemoryJobUpdate;
const memorySignal = {
  targetRef: "curated:1",
  queryFingerprint: "salted",
  day: "2026-10-24",
  bestScore: 0.8,
  hitCount: 1,
} satisfies MemoryRecallSignal;
void [memorySource, memoryClaim, memoryUpdate, memorySignal];
// @ts-expect-error Memory candidate importance retains the literal source range.
const badImportance: MemoryCandidate["importance"] = 0;
const badMemoryPatch: MemoryJobUpdate["patch"] = {
  status: "completed",
  // @ts-expect-error Job patches cannot replace owner authority.
  ownerId: "other",
};
const badMemorySignal: MemoryRecallSignal = {
  ...memorySignal,
  // @ts-expect-error Recall records never include raw query text.
  query: "sensitive",
};
void [badImportance, badMemoryPatch, badMemorySignal];

const searchDocument = {
  path: "A.md",
  name: "A",
  extension: "md",
  checksum: "hash",
  content: "hello🪨",
  tags: [],
  tagParts: [],
  tagHierarchy: [],
  chunks: [
    {
      id: "A#0",
      text: "hello🪨",
      startOffset: 0,
      endOffset: 7,
      kind: "paragraph",
      embedding: {
        status: "ready",
        modelId: "model",
        vector: [1, 0],
        dimensions: 2,
      },
    },
  ],
  sourceMetadata: {
    rawTags: [],
    frontmatter: null,
    headings: [],
    sections: [
      {
        type: "paragraph",
        position: { start: { offset: 0 }, end: { offset: 7 } },
      },
    ],
    chunking: {
      targetChars: 100,
      breakpointWindowChars: 10,
      breakpointDecay: 0.5,
    },
  },
} satisfies SearchDocumentRecord;
const searchManifest = {
  rows: [{ path: "A.md", checksum: "hash" }],
  nextCursor: "A.md",
} satisfies SearchManifestPage;
const searchStats = {
  documentCount: 1,
  chunkCount: 1,
  readyChunkCount: 1,
  pendingChunkCount: 0,
  errorChunkCount: 0,
  lastError: null,
} satisfies SearchIndexStats;
const badSearchQuery: SearchManifestQuery = {
  // @ts-expect-error The public Search query cannot expose SQL.
  sql: "SELECT * FROM search_docs",
};
const badSearchDocument: SearchDocumentRecord = {
  ...searchDocument,
  // @ts-expect-error Search bodies cannot choose native storage paths.
  databasePath: "/tmp/other.db",
};
const searchOptions: SearchOptions = {
  mode: "lexical",
  limit: 10,
  pathPrefix: "Projects",
  caseSensitive: true,
  includeDiagnostics: true,
  sourceProviderIds: ["markdown"],
};
const badRetrieval: SearchOptions = {
  // @ts-expect-error Search retrieval cannot carry SQL.
  sql: "SELECT * FROM search_docs",
};
const badMode: SearchOptions = {
  // @ts-expect-error Only declared retrieval modes are admitted.
  mode: "execute",
};
function resultPath(result: SearchResult): string {
  return result.document.path;
}
void [
  searchDocument,
  searchManifest,
  searchStats,
  badSearchQuery,
  badSearchDocument,
  searchOptions,
  badRetrieval,
  badMode,
  resultPath,
];

function indexedTaskSelection(
  input: CoreIndexedFile,
  task: TaskRecord,
): IndexedFile[] {
  return [input, { ...input, task: null }, { ...input, task }];
}
const invalidIndexedTask: IndexedFile = {
  ...({} as CoreIndexedFile),
  // @ts-expect-error Task selection admits a record or null, never a command.
  task: "execute",
};
const invalidIndexedSql: IndexedFile = {
  ...({} as CoreIndexedFile),
  // @ts-expect-error Indexed operations cannot carry raw SQL.
  sql: "DELETE FROM files",
};
void [indexedTaskSelection, invalidIndexedTask, invalidIndexedSql];

const nativeEmbeddingOptions: SearchEmbeddingProviderConfig = {
  kind: "transformers-js",
  modelId: "Xenova/all-MiniLM-L6-v2",
  dimensions: 384,
};
const invalidEmbeddingPath: SearchEmbeddingProviderConfig = {
  ...nativeEmbeddingOptions,
  // @ts-expect-error Browser configuration cannot select native model files.
  localModelPath: "/private/models",
};
const invalidEmbeddingRuntime: SearchEmbeddingProviderConfig = {
  ...nativeEmbeddingOptions,
  // @ts-expect-error Native shared-library admission is host authority.
  runtime: "/private/runtime.dylib",
};
void [nativeEmbeddingOptions, invalidEmbeddingPath, invalidEmbeddingRuntime];
