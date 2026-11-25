use super::super::{
    State,
    errors::{Failure, blocking},
    routes::body,
};
use crate::host::Principal;
use axum::{
    Extension, Json,
    extract::{Path, State as AxumState, rejection::JsonRejection},
    http::header,
};
use lapis_app_core::credentials::{PluginCredentialCommand, PluginCredentialReply};
use uuid::Uuid;

pub(crate) async fn grants(
    AxumState(state): AxumState<State>,
    Path(client): Path<Uuid>,
    Extension(principal): Extension<Principal>,
) -> Result<([(header::HeaderName, &'static str); 1], Json<Vec<String>>), Failure> {
    Ok((
        [(header::CACHE_CONTROL, "no-store")],
        Json(state.host.plugin_credential_grants(client, principal)?),
    ))
}

pub(crate) async fn call(
    AxumState(state): AxumState<State>,
    Path(client): Path<Uuid>,
    Extension(principal): Extension<Principal>,
    input: Result<Json<PluginCredentialCommand>, JsonRejection>,
) -> Result<
    (
        [(header::HeaderName, &'static str); 1],
        Json<PluginCredentialReply>,
    ),
    Failure,
> {
    let command = body(input)?;
    let reply = blocking(move || state.host.plugin_credentials(client, principal, command)).await?;
    Ok(([(header::CACHE_CONTROL, "no-store")], Json(reply)))
}
