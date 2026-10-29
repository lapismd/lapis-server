mod client;
mod definitions;
mod query;
mod query_evaluation;
mod query_page;
mod reads;
mod source_paths;
pub(super) mod values;
mod writes;

use super::{invalid, store::Store};
use crate::services::Result;
use lapis_app_protocol::database::projections::queries::*;
use lapis_app_protocol::database::projections::*;
use serde::Serialize;

#[derive(Serialize)]
pub(super) enum Action {
    Register(ProjectionDefinition),
    Unregister(String),
    Replace(ProjectionSource),
    Error(ProjectionSourceError),
    Delete(String, String),
}
#[derive(Serialize)]
pub(super) enum ReadAction {
    NativeState(String, String),
    NativeSourcePaths(String, Option<String>),
    Get(String, String, Option<String>),
    Query(String, ProjectionQuery, Option<String>),
    Related(ProjectionRelatedQuery, Option<String>),
}
pub(super) enum ReadValue {
    NativeState(NativeProjectionState),
    NativeSourcePaths(Vec<String>),
    Row(Option<serde_json::Value>),
    Query(ProjectionQueryResult),
}
pub(crate) struct NativeProjectionState {
    pub definition: Option<ProjectionDefinition>,
    pub source: Option<NativeSourceState>,
}
pub(crate) struct NativeSourceState {
    pub hash: String,
    pub schema_version: f64,
    pub config_hash: String,
    pub status: String,
    pub error: Option<String>,
}
impl Store {
    pub async fn projection(&mut self, action: Action) -> Result<()> {
        self.begin().await?;
        let result = self.write_projection(action).await;
        self.finish(result).await
    }
}
fn write_access(id: &str, writer: Option<&str>) -> Result<()> {
    if writer.is_some_and(|w| !w.is_empty() && id.split('/').next() != Some(w)) {
        return Err(invalid("Plugin cannot write another projection namespace"));
    }
    Ok(())
}
