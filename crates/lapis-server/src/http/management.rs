use super::{State, errors::Failure, routes::body};
use crate::host::ManagementCommand;
use axum::{
    Json,
    extract::{Path, State as AxumState, rejection::JsonRejection},
    http::StatusCode,
};
use lapis_app_protocol::{ManagedVault, browser::VaultName};
use uuid::Uuid;

pub(super) async fn create(
    AxumState(state): AxumState<State>,
    input: Result<Json<VaultName>, JsonRejection>,
) -> Result<(StatusCode, Json<ManagedVault>), Failure> {
    let vault = state
        .host
        .manage(ManagementCommand::Create(body(input)?.name))
        .await?;
    Ok((StatusCode::CREATED, Json(vault)))
}
pub(super) async fn rename(
    AxumState(state): AxumState<State>,
    Path(id): Path<Uuid>,
    input: Result<Json<VaultName>, JsonRejection>,
) -> Result<Json<ManagedVault>, Failure> {
    Ok(Json(
        state
            .host
            .manage(ManagementCommand::Rename(id, body(input)?.name))
            .await?,
    ))
}
pub(super) async fn archive(
    AxumState(state): AxumState<State>,
    Path(id): Path<Uuid>,
) -> Result<Json<ManagedVault>, Failure> {
    Ok(Json(
        state.host.manage(ManagementCommand::Archive(id)).await?,
    ))
}
pub(super) async fn restore(
    AxumState(state): AxumState<State>,
    Path(id): Path<Uuid>,
) -> Result<Json<ManagedVault>, Failure> {
    Ok(Json(
        state.host.manage(ManagementCommand::Restore(id)).await?,
    ))
}
