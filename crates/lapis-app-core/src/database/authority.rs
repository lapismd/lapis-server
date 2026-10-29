//! Client-generated writes are fenced before native selection, never by caller lifetime.
#[cfg(test)]
mod tests;
use super::Database;
use crate::services::{Error, Result};
use lapis_app_protocol::{
    ErrorCode,
    database::call::{DatabaseCall, DatabaseMethod, DatabaseValue},
};
use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
};

#[derive(Default)]
pub(super) struct Authority {
    metadata: bool,
    search: bool,
    projections: BTreeSet<String>,
}
impl Database {
    pub(crate) async fn claim_native_metadata(&self) {
        // Fair exclusive acquisition drains every previously admitted complete
        // generated call, including a multi-stage call whose waiter detached.
        self.authority.write().await.metadata = true;
    }
    pub(crate) async fn claim_native_search(&self) {
        self.authority.write().await.search = true;
    }
    pub(crate) async fn release_native_search(&self) {
        self.authority.write().await.search = false;
    }
    pub(crate) async fn claim_native_projection(&self, id: &str) {
        self.authority.write().await.projections.insert(id.into());
    }
    pub(crate) async fn release_native_projection(&self, id: &str) {
        self.authority.write().await.projections.remove(id);
    }
    pub(crate) async fn call_from_client(&self, request: DatabaseCall) -> Result<DatabaseValue> {
        if !Authority::generated(&request) {
            return self.call(request).await;
        }
        let guard = self.authority.clone().read_owned().await;
        if guard.rejects(&request) {
            return Err(Error::new(
                ErrorCode::PermissionDenied,
                "Generated index writes belong to the selected native service",
            ));
        }
        let database = self.admitted_facade()?;
        // A dropped response must not release authority while an accepted
        // facade is still submitting its later publication stages.
        tokio::spawn(async move {
            let _authority = guard;
            database.call(request).await
        })
        .await
        .map_err(|_| {
            Error::new(
                ErrorCode::Io,
                "Database outcome is unavailable; do not retry",
            )
        })?
    }

    fn admitted_facade(&self) -> Result<Self> {
        let sender = self
            .admission
            .lock()
            .map_err(|_| super::closed())?
            .as_ref()
            .ok_or_else(super::closed)?
            .clone();
        let mut database = self.clone();
        // Closing fences public callers; this accepted facade keeps the same
        // bounded worker alive until every stage has completed.
        database.admission = Arc::new(Mutex::new(Some(sender)));
        Ok(database)
    }
}
impl Authority {
    fn generated(call: &DatabaseCall) -> bool {
        use DatabaseMethod::*;
        match call.method {
            UpsertIndexedFile | DeleteIndexedFile | RenameIndexedFile | SaveMetadataSnapshot
            | UpsertTaskProjection | DeleteTaskProjection | UpsertSearchDocument
            | DeleteSearchDocument => true,
            RegisterProjectionDefinition
            | ReplaceProjectionSource
            | MarkProjectionSourceError
            | UnregisterProjectionDefinition
            | DeleteProjectionSource => Self::projection(call)
                .is_some_and(|id| matches!(id, "tasks/task" | "tasks/occurrence" | "docs/note")),
            SetMeta | TransactMeta => Self::checkpoint(call, true, true),
            _ => false,
        }
    }
    fn first(call: &DatabaseCall) -> Option<&serde_json::Value> {
        match call.args.first() {
            Some(DatabaseValue::Value { value }) => Some(value),
            _ => None,
        }
    }
    fn projection(call: &DatabaseCall) -> Option<&str> {
        use DatabaseMethod::*;
        let first = Self::first(call)?;
        match call.method {
            RegisterProjectionDefinition | ReplaceProjectionSource | MarkProjectionSourceError => {
                first
                    .get("projectionId")
                    .and_then(serde_json::Value::as_str)
            }
            UnregisterProjectionDefinition | DeleteProjectionSource => first.as_str(),
            _ => None,
        }
    }
    fn checkpoint(call: &DatabaseCall, metadata: bool, search: bool) -> bool {
        use DatabaseMethod::*;
        let Some(first) = Self::first(call) else {
            return false;
        };
        let reserved = |key: &str| {
            metadata && key == "metadata-cache.reconcile-checkpoint.v1"
                || search && key == "search.reconcile-checkpoint.v1"
        };
        match call.method {
            SetMeta => first.as_str().is_some_and(reserved),
            TransactMeta => first["mutations"].as_array().is_some_and(|mutations| {
                mutations
                    .iter()
                    .any(|mutation| mutation["key"].as_str().is_some_and(reserved))
            }),
            _ => false,
        }
    }
    fn rejects(&self, call: &DatabaseCall) -> bool {
        use DatabaseMethod::*;
        if self.metadata
            && matches!(
                call.method,
                UpsertIndexedFile | DeleteIndexedFile | RenameIndexedFile | SaveMetadataSnapshot
            )
        {
            return true;
        }
        if matches!(call.method, UpsertTaskProjection | DeleteTaskProjection)
            && self.projections.contains("tasks/task")
        {
            return true;
        }
        if self.search && matches!(call.method, UpsertSearchDocument | DeleteSearchDocument) {
            return true;
        }
        Self::projection(call).is_some_and(|id| self.projections.contains(id))
            || Self::checkpoint(call, self.metadata, self.search)
    }
}
