use super::{
    State,
    errors::{Failure, blocking},
};
use crate::host::Principal;
use axum::{
    Extension, Json,
    extract::{Path, State as AxumState, rejection::JsonRejection},
    http::StatusCode,
};
use lapis_app_protocol::{
    AttachClient, ClientDescriptor, ErrorCode, ManagedVault, Outcome, Reply, Request,
};
use uuid::Uuid;

pub(super) fn body<T>(body: Result<Json<T>, JsonRejection>) -> Result<T, Failure> {
    body.map(|Json(value)| value).map_err(|rejection| {
        Failure(
            rejection.status(),
            crate::host::error(ErrorCode::InvalidRequest, "Invalid command body"),
        )
    })
}
pub(super) async fn vaults(
    AxumState(state): AxumState<State>,
) -> Result<Json<Vec<ManagedVault>>, Failure> {
    Ok(Json(state.host.vaults()?))
}
pub(super) async fn attach(
    AxumState(state): AxumState<State>,
    Path(vault): Path<Uuid>,
    Extension(principal): Extension<Principal>,
    input: Result<Json<AttachClient>, JsonRejection>,
) -> Result<Json<ClientDescriptor>, Failure> {
    if body(input)?.protocol_version != lapis_app_protocol::PROTOCOL_MAJOR {
        return Err(Failure::new(
            ErrorCode::ProtocolMismatch,
            "Reload the matching application client",
        ));
    }
    let host = state.host.clone();
    let descriptor = blocking(move || host.attach(vault, principal)).await?;
    if let Err(error) = state
        .host
        .prepare_client(descriptor.client_id, principal)
        .await
    {
        let host = state.host.clone();
        let client = descriptor.client_id;
        let _ = blocking(move || host.detach(client, principal)).await;
        return Err(error.into());
    }
    Ok(Json(descriptor))
}
pub(super) async fn execute(
    AxumState(state): AxumState<State>,
    Path(client): Path<Uuid>,
    Extension(principal): Extension<Principal>,
    input: Result<Json<Request>, JsonRejection>,
) -> Result<(StatusCode, Json<Reply>), Failure> {
    let request = body(input)?;
    let reply = blocking(move || state.host.execute(client, principal, request)).await?;
    let status = match &reply.outcome {
        Outcome::Ok { .. } => StatusCode::OK,
        Outcome::Error { error } => Failure::from(error.clone()).0,
    };
    Ok((status, Json(reply)))
}
pub(super) async fn detach(
    AxumState(state): AxumState<State>,
    Path(client): Path<Uuid>,
    Extension(principal): Extension<Principal>,
) -> Result<StatusCode, Failure> {
    blocking(move || state.host.detach(client, principal)).await?;
    Ok(StatusCode::NO_CONTENT)
}
pub(super) async fn heartbeat(
    AxumState(state): AxumState<State>,
    Path(client): Path<Uuid>,
    Extension(principal): Extension<Principal>,
) -> Result<StatusCode, Failure> {
    blocking(move || state.host.heartbeat(client, principal)).await?;
    Ok(StatusCode::NO_CONTENT)
}
pub(super) async fn close(
    AxumState(state): AxumState<State>,
    Path(vault): Path<Uuid>,
) -> Result<StatusCode, Failure> {
    blocking(move || state.host.close(vault)).await?;
    Ok(StatusCode::NO_CONTENT)
}
