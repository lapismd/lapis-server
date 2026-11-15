use super::{
    State,
    errors::Failure,
    sync_account::{input, native, store},
};
use crate::control::{
    AccountConnectionStatus, DiscoveryCredentialStatus, NativeAccountInspection, now_ms,
};
use axum::{
    Json,
    extract::{Path, Request, State as AxumState},
};
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Proof {
    verifier: String,
}
pub(super) async fn inspect(
    AxumState(state): AxumState<State>,
    Path(id): Path<Uuid>,
    request: Request,
) -> Result<Json<Value>, Failure> {
    native(&state, &request)?;
    let body: Proof = input(request).await?;
    let inspected = state
        .auth
        .control
        .as_ref()
        .ok_or_else(Failure::unauthorized)?
        .inspect_native_account_connection(id, &body.verifier, now_ms().map_err(store)?)
        .await
        .map_err(store)?;
    Ok(Json(public(inspected)))
}
pub(super) async fn cancel(
    AxumState(state): AxumState<State>,
    Path(id): Path<Uuid>,
    request: Request,
) -> Result<Json<Value>, Failure> {
    native(&state, &request)?;
    let body: Proof = input(request).await?;
    let inspected = state
        .auth
        .control
        .as_ref()
        .ok_or_else(Failure::unauthorized)?
        .cancel_native_account_connection(id, &body.verifier, now_ms().map_err(store)?)
        .await
        .map_err(store)?;
    Ok(Json(public(inspected)))
}
fn public(value: NativeAccountInspection) -> Value {
    let status = match value.status {
        AccountConnectionStatus::Pending => "pending",
        AccountConnectionStatus::Approved => "approved",
        AccountConnectionStatus::Rejected => "rejected",
        AccountConnectionStatus::Consumed => "consumed",
        AccountConnectionStatus::Expired => "expired",
    };
    let credential = match value.credential_status {
        DiscoveryCredentialStatus::NotIssued => "not-issued",
        DiscoveryCredentialStatus::Active => "active",
        DiscoveryCredentialStatus::Revoked => "revoked",
        DiscoveryCredentialStatus::Expired => "expired",
        DiscoveryCredentialStatus::Unavailable => "unavailable",
    };
    json!({"connection_id":value.connection_id,"client_id":value.client_id,"expires_at":value.expires_at,"status":status,"credential_status":credential})
}
