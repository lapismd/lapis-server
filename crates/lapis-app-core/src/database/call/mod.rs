mod indexed;
mod memory;
mod projections;
mod state;

use super::{Database, encoded, invalid};
use crate::services::Result;
use lapis_app_protocol::database::call::{DatabaseCall, DatabaseMethod::*, DatabaseValue};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

impl Database {
    /// Hosts admit the vault first. Each call uses the existing accepted-work owner.
    pub async fn call(&self, request: DatabaseCall) -> Result<DatabaseValue> {
        encoded(&request)?;
        if request.args.len() > 4 {
            return Err(invalid(
                "Application database calls accept at most four arguments",
            ));
        }
        let args = Arguments(request.args);
        let result = match request.method {
            Migrate
            | BeginSearchIndexingBatch
            | EndSearchIndexingBatch
            | GetMeta
            | SetMeta
            | ScanMeta
            | TransactMeta
            | GetNotebookState
            | SetNotebookState
            | DeleteNotebookState
            | LoadMetadataSnapshot
            | SaveMetadataSnapshot
            | GetFileHistory
            | StoreFileHistoryRevision
            | ListNotifications
            | UpsertNotification
            | MarkNotificationRead
            | ClearNotification
            | ClearAllNotifications
            | GetChangeRevision => state::call(self, request.method, &args).await,
            GetMemorySourceState
            | ListMemorySourceStates
            | UpsertMemorySourceState
            | DeleteMemorySourceState
            | UpsertMemoryCandidate
            | QueryMemoryCandidates
            | DeleteMemoryCandidatesByConversation
            | RecordMemoryRecallSignal
            | ListMemoryRecallSignals
            | ClaimMemoryJob
            | UpdateMemoryJob
            | ListMemoryJobs
            | ClearMemoryDerivedState => memory::call(self, request.method, &args).await,
            ConfigureSearchEmbeddingProvider
            | GetSearchEmbeddingProvider
            | GetSearchEmbeddingRuntimeStatus
            | GetSearchIndexStats
            | UpsertIndexedFile
            | GetIndexedFile
            | ListIndexedFileManifest
            | QueryIndexedMetadata
            | QueryIndexedMetadataPage
            | QueryMetadataFacets
            | QueryMetadataLinks
            | DeleteIndexedFile
            | RenameIndexedFile
            | UpsertSearchDocument
            | DeleteSearchDocument
            | GetSearchDocument
            | ListSearchDocumentManifest
            | ListSearchDocuments
            | RebuildSearchIndex
            | SearchDocuments
            | SearchDocumentPaths => indexed::call(self, request.method, &args).await,
            UpsertTaskProjection
            | DeleteTaskProjection
            | QueryTasks
            | GetTaskRow
            | ListChildLinks
            | ListTaskDescendants
            | RegisterProjectionDefinition
            | UnregisterProjectionDefinition
            | ReplaceProjectionSource
            | MarkProjectionSourceError
            | DeleteProjectionSource
            | QueryProjection
            | GetProjectionRow
            | QueryRelated => projections::call(self, request.method, &args).await,
        }?;
        encoded(&result)?;
        Ok(result)
    }
}

struct Arguments(Vec<DatabaseValue>);
impl Arguments {
    fn raw(&self, index: usize) -> Option<&Value> {
        match self.0.get(index) {
            Some(DatabaseValue::Value { value }) => Some(value),
            _ => None,
        }
    }
    fn required<T: DeserializeOwned>(&self, index: usize) -> Result<T> {
        let value = self
            .raw(index)
            .ok_or_else(|| invalid("Missing database argument"))?;
        serde_json::from_value(value.clone()).map_err(|_| invalid("Invalid database argument"))
    }
    fn default<T: DeserializeOwned + Default>(&self, index: usize) -> Result<T> {
        match self.raw(index) {
            Some(_) => self.required(index),
            None => Ok(T::default()),
        }
    }
    fn text(&self, index: usize) -> Result<&str> {
        self.raw(index)
            .and_then(Value::as_str)
            .ok_or_else(|| invalid("Database argument must be text"))
    }
    fn optional_text(&self, index: usize) -> Result<Option<&str>> {
        match self.raw(index) {
            Some(value) if !value.is_null() => self.text(index).map(Some),
            _ => Ok(None),
        }
    }
}

fn value<T: Serialize>(result: Result<T>) -> Result<DatabaseValue> {
    result.and_then(|value| {
        serde_json::to_value(value)
            .map(|value| DatabaseValue::Value { value })
            .map_err(|_| invalid("Invalid application database result"))
    })
}
fn optional<T: Serialize>(result: Result<Option<T>>) -> Result<DatabaseValue> {
    match result? {
        Some(row) => value(Ok(row)),
        None => Ok(DatabaseValue::Undefined {}),
    }
}
fn done(result: Result<()>) -> Result<DatabaseValue> {
    result.map(|_| DatabaseValue::Undefined {})
}
