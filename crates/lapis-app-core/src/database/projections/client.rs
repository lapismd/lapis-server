use super::{Action, NativeProjectionState, ReadAction, ReadValue, write_access};
use crate::database::{Database, Operation, Output, invalid};
use crate::services::Result;
use lapis_app_protocol::database::projections::queries::*;
use lapis_app_protocol::database::projections::*;

impl Database {
    pub(crate) async fn native_projection_state(
        &self,
        id: &str,
        path: &str,
    ) -> Result<NativeProjectionState> {
        match self
            .submit(Operation::ProjectionRead(ReadAction::NativeState(
                id.into(),
                path.into(),
            )))
            .await?
        {
            Output::ProjectionRead(ReadValue::NativeState(value)) => Ok(value),
            _ => unreachable!("worker output matches operation"),
        }
    }
    pub async fn query_projection(
        &self,
        id: &str,
        query: ProjectionQuery,
        reader: Option<&str>,
    ) -> Result<ProjectionQueryResult> {
        self.projection_query(ReadAction::Query(
            id.into(),
            query,
            reader.map(str::to_owned),
        ))
        .await
    }
    pub async fn query_related(
        &self,
        query: ProjectionRelatedQuery,
        reader: Option<&str>,
    ) -> Result<ProjectionQueryResult> {
        self.projection_query(ReadAction::Related(query, reader.map(str::to_owned)))
            .await
    }
    async fn projection_query(&self, action: ReadAction) -> Result<ProjectionQueryResult> {
        match self.submit(Operation::ProjectionRead(action)).await? {
            Output::ProjectionRead(ReadValue::Query(value)) => Ok(value),
            _ => unreachable!("worker output matches operation"),
        }
    }
    pub async fn get_projection_row(
        &self,
        id: &str,
        row: &str,
        reader: Option<&str>,
    ) -> Result<Option<serde_json::Value>> {
        match self
            .submit(Operation::ProjectionRead(ReadAction::Get(
                id.into(),
                row.into(),
                reader.map(str::to_owned),
            )))
            .await?
        {
            Output::ProjectionRead(ReadValue::Row(value)) => Ok(value),
            _ => unreachable!("worker output matches operation"),
        }
    }
    pub async fn register_projection_definition(
        &self,
        definition: ProjectionDefinition,
    ) -> Result<()> {
        if !definition.schema_version.is_finite() || !definition.updated_at.is_finite() {
            return Err(invalid("Invalid projection definition number"));
        }
        self.submit(Operation::Projection(Action::Register(definition)))
            .await
            .map(|_| ())
    }
    pub async fn unregister_projection_definition(&self, id: &str) -> Result<()> {
        self.submit(Operation::Projection(Action::Unregister(id.into())))
            .await
            .map(|_| ())
    }
    pub async fn replace_projection_source(&self, input: ProjectionSource) -> Result<()> {
        write_access(&input.projection_id, input.writer_plugin_id.as_deref())?;
        if input.rows.len() > 64 {
            return Err(invalid("Projection source exceeds 64 rows"));
        }
        if input
            .rows
            .iter()
            .any(|r| r.ordinal.is_some_and(|n| !n.is_finite()))
            || input.edges.iter().any(|e| !e.ordinal.is_finite())
        {
            return Err(invalid("Invalid projection ordinal"));
        }
        self.submit(Operation::Projection(Action::Replace(input)))
            .await
            .map(|_| ())
    }
    pub async fn mark_projection_source_error(&self, input: ProjectionSourceError) -> Result<()> {
        write_access(&input.projection_id, input.writer_plugin_id.as_deref())?;
        self.submit(Operation::Projection(Action::Error(input)))
            .await
            .map(|_| ())
    }
    pub async fn delete_projection_source(
        &self,
        id: &str,
        path: &str,
        writer: Option<&str>,
    ) -> Result<()> {
        write_access(id, writer)?;
        self.submit(Operation::Projection(Action::Delete(
            id.into(),
            path.into(),
        )))
        .await
        .map(|_| ())
    }
}
