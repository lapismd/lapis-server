use super::{State, auth, errors::Failure, sync_account::store};
use crate::control::{AccountCredential, now_ms};
use axum::{Json, extract::State as AxumState, http::HeaderMap};
use serde_json::{Value, json};

async fn credential(state: &State, headers: &HeaderMap) -> Result<AccountCredential, Failure> {
    auth::origin(headers, &state.origin, false)?;
    auth::version(headers, false)?;
    if headers.contains_key("cookie") || headers.get_all("authorization").iter().count() != 1 {
        return Err(Failure::unauthorized());
    }
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(Failure::unauthorized)?;
    state
        .auth
        .control
        .as_ref()
        .ok_or_else(Failure::unauthorized)?
        .resolve_account_credential(token, now_ms().map_err(store)?)
        .await
        .map_err(store)?
        .ok_or_else(Failure::unauthorized)
}

pub(super) async fn vaults(
    AxumState(state): AxumState<State>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    credential(&state, &headers).await?;
    let control = state
        .auth
        .control
        .as_ref()
        .ok_or_else(Failure::unauthorized)?;
    let vaults = control.discover_account_vaults().await.map_err(store)?;
    // Admission remains an independent enrollment step. Discovery discloses
    // only active managed vault names/identities, never paths or content.
    Ok(Json(json!({"vaults":vaults.into_iter()
        .map(|v| json!({"vault_id":v.vault_id,"name":v.name})).collect::<Vec<_>>()})))
}

pub(super) async fn sign_out(
    AxumState(state): AxumState<State>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    let issued = credential(&state, &headers).await?;
    state
        .auth
        .control
        .as_ref()
        .ok_or_else(Failure::unauthorized)?
        .revoke_account_credential(issued.id, now_ms().map_err(store)?)
        .await
        .map_err(store)?;
    Ok(Json(json!({"ok":true})))
}
