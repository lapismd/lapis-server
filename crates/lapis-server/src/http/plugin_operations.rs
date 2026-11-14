use super::{State, errors::Failure, routes::body};
use crate::host::Principal;
use axum::{
    Extension, Json,
    extract::{Path, State as AxumState, rejection::JsonRejection},
};
use lapis_app_protocol::operations::*;
use uuid::Uuid;

pub(super) async fn catalog(
    AxumState(state): AxumState<State>,
    Path(client): Path<Uuid>,
    Extension(principal): Extension<Principal>,
) -> Result<Json<OperationCatalog>, Failure> {
    state
        .host
        .plugin_operation_catalog(client, principal)
        .await
        .map(Json)
        .map_err(Into::into)
}
pub(super) async fn invoke(
    AxumState(state): AxumState<State>,
    Path((client, plugin, operation)): Path<(Uuid, String, String)>,
    Extension(principal): Extension<Principal>,
    input: Result<Json<OperationRequest>, JsonRejection>,
) -> Result<Json<OperationReply>, Failure> {
    state
        .host
        .plugin_operation(client, principal, plugin, operation, body(input)?)
        .await
        .map(Json)
        .map_err(Into::into)
}
