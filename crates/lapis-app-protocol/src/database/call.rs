//! High-level methods only; native paths, SQL and lifecycle admission stay host-owned.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase")]
pub enum DatabaseMethod {
    Migrate,
    BeginSearchIndexingBatch,
    EndSearchIndexingBatch,
    ConfigureSearchEmbeddingProvider,
    GetSearchEmbeddingProvider,
    GetSearchEmbeddingRuntimeStatus,
    GetSearchIndexStats,
    GetMeta,
    SetMeta,
    ScanMeta,
    TransactMeta,
    GetNotebookState,
    SetNotebookState,
    DeleteNotebookState,
    LoadMetadataSnapshot,
    SaveMetadataSnapshot,
    GetFileHistory,
    StoreFileHistoryRevision,
    ListNotifications,
    UpsertNotification,
    MarkNotificationRead,
    ClearNotification,
    ClearAllNotifications,
    GetMemorySourceState,
    ListMemorySourceStates,
    UpsertMemorySourceState,
    DeleteMemorySourceState,
    UpsertMemoryCandidate,
    QueryMemoryCandidates,
    DeleteMemoryCandidatesByConversation,
    RecordMemoryRecallSignal,
    ListMemoryRecallSignals,
    ClaimMemoryJob,
    UpdateMemoryJob,
    ListMemoryJobs,
    ClearMemoryDerivedState,
    GetChangeRevision,
    UpsertIndexedFile,
    GetIndexedFile,
    ListIndexedFileManifest,
    QueryIndexedMetadata,
    QueryIndexedMetadataPage,
    QueryMetadataFacets,
    QueryMetadataLinks,
    DeleteIndexedFile,
    RenameIndexedFile,
    UpsertSearchDocument,
    DeleteSearchDocument,
    GetSearchDocument,
    ListSearchDocumentManifest,
    ListSearchDocuments,
    RebuildSearchIndex,
    SearchDocuments,
    SearchDocumentPaths,
    UpsertTaskProjection,
    DeleteTaskProjection,
    QueryTasks,
    GetTaskRow,
    ListChildLinks,
    ListTaskDescendants,
    RegisterProjectionDefinition,
    UnregisterProjectionDefinition,
    ReplaceProjectionSource,
    MarkProjectionSourceError,
    DeleteProjectionSource,
    QueryProjection,
    GetProjectionRow,
    QueryRelated,
}

/// Explicit tags preserve undefined array slots and undefined results over JSON.
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[ts(tag = "kind", rename_all = "snake_case")]
pub enum DatabaseValue {
    Undefined {},
    Value {
        #[ts(type = "unknown")]
        value: Value,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct DatabaseCall {
    pub method: DatabaseMethod,
    #[serde(default)]
    #[schemars(length(max = 4))]
    #[ts(as = "Option<Vec<DatabaseValue>>", optional)]
    pub args: Vec<DatabaseValue>,
}
