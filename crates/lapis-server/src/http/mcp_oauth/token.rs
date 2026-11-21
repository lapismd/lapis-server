use super::*;
use crate::{
    control::{
        mcp_oauth::{Client, Refresh, TokenPair},
        now_ms, passwords,
    },
    host::Host,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use subtle::ConstantTimeEq;

pub(super) async fn exchange(
    AxumState(state): AxumState<State>,
    request: Request,
) -> Result<Json<TokenPair>, OAuthError> {
    let (control, _) = resource_ready(&state, request.headers())?;
    let (fields, client) = authenticate(&state, control, request).await?;
    tokio::spawn(async move { exchange_authorized(state, fields, client).await })
        .await
        .map_err(|_| OAuthError::busy())?
}

async fn exchange_authorized(
    state: State,
    fields: BTreeMap<String, String>,
    client: Client,
) -> Result<Json<TokenPair>, OAuthError> {
    let control = state
        .auth
        .control
        .as_ref()
        .ok_or_else(OAuthError::not_found)?;
    let oauth = state.oauth.as_ref().ok_or_else(OAuthError::not_found)?;
    let _accepted = state
        .host
        .content
        .accept()
        .map_err(|_| OAuthError::busy())?;
    let _gate = state.host.content.gate.lock().await;
    if !enabled(&state) {
        return Err(OAuthError::not_found());
    }
    let audience = resource(&state);
    if fields
        .get("resource")
        .is_some_and(|resource| resource != &audience)
    {
        return Err(OAuthError::invalid_target());
    }
    let now = now_ms().map_err(OAuthError::store)?;
    match fields.get("grant_type").map(String::as_str) {
        Some("authorization_code") => {
            if fields.get("resource") != Some(&audience) {
                return Err(OAuthError::invalid_target());
            }
            let verifier = fields
                .get("code_verifier")
                .ok_or_else(OAuthError::invalid_grant)?;
            if !(43..=128).contains(&verifier.len())
                || !verifier
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-._~".contains(&b))
            {
                return Err(OAuthError::invalid_grant());
            }
            let code = fields.get("code").ok_or_else(OAuthError::invalid_grant)?;
            let mut pending = oauth.pending.lock().await;
            let digest = passwords::digest(code);
            let (deadline, entry) = pending
                .codes
                .get(&digest)
                .ok_or_else(OAuthError::invalid_grant)?;
            let presented = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
            if *deadline <= std::time::Instant::now()
                || entry.generation != state.host.content.generation()
                || entry.client != client.id
                || fields.get("redirect_uri") != Some(&entry.redirect)
                || !bool::from(presented.as_bytes().ct_eq(entry.challenge.as_bytes()))
            {
                return Err(OAuthError::invalid_grant());
            }
            let (_, entry) = pending
                .codes
                .remove(&digest)
                .ok_or_else(OAuthError::invalid_grant)?;
            drop(pending);
            let issued = control
                .issue_mcp(&entry.grant, now)
                .await
                .map_err(OAuthError::store)?
                .ok_or_else(OAuthError::busy)?;
            Ok(Json(issued))
        }
        Some("refresh_token") => {
            let token = fields
                .get("refresh_token")
                .filter(|value| value.starts_with("lor_") && value.len() == 47)
                .ok_or_else(OAuthError::invalid_grant)?;
            if fields.contains_key("scope") {
                return Err(OAuthError(StatusCode::BAD_REQUEST, "invalid_scope"));
            }
            match control
                .refresh_mcp(&client.id, &audience, token, now)
                .await
                .map_err(OAuthError::store)?
            {
                Refresh::Issued(id, pair) => {
                    retire_lease(state.host.clone(), id).await?;
                    Ok(Json(pair))
                }
                Refresh::Replayed(id) => {
                    retire_lease(state.host.clone(), id).await?;
                    Err(OAuthError::invalid_grant())
                }
                Refresh::Invalid => Err(OAuthError::invalid_grant()),
            }
        }
        _ => Err(OAuthError(
            StatusCode::BAD_REQUEST,
            "unsupported_grant_type",
        )),
    }
}

pub(super) async fn revoke(
    AxumState(state): AxumState<State>,
    request: Request,
) -> Result<StatusCode, OAuthError> {
    let (control, _) = resource_ready(&state, request.headers())?;
    let (fields, client) = authenticate(&state, control, request).await?;
    let token = fields
        .get("token")
        .cloned()
        .ok_or_else(OAuthError::invalid_request)?;
    tokio::spawn(async move { revoke_authorized(state, client, token).await })
        .await
        .map_err(|_| OAuthError::busy())?
}

async fn revoke_authorized(
    state: State,
    client: Client,
    token: String,
) -> Result<StatusCode, OAuthError> {
    let control = state
        .auth
        .control
        .as_ref()
        .ok_or_else(OAuthError::not_found)?;
    let _accepted = state
        .host
        .content
        .accept()
        .map_err(|_| OAuthError::busy())?;
    let _gate = state.host.content.gate.lock().await;
    let now = now_ms().map_err(OAuthError::store)?;
    if let Some(id) = control
        .revoke_mcp_token(&client.id, &resource(&state), &token, now)
        .await
        .map_err(OAuthError::store)?
    {
        retire_lease(state.host.clone(), id).await?;
    }
    Ok(StatusCode::OK)
}

async fn authenticate(
    state: &State,
    control: &Control,
    request: Request,
) -> Result<(BTreeMap<String, String>, Client), OAuthError> {
    let _permit = state
        .host
        .content
        .oauth_admission
        .clone()
        .try_acquire_owned()
        .map_err(|_| OAuthError::busy())?;
    let basic = client::basic(request.headers())?;
    let form = http_body(request, "application/x-www-form-urlencoded").await?;
    let mut fields = params(&form)?;
    let (id, secret, used_basic) = match basic {
        Some((id, secret))
            if !fields.contains_key("client_secret")
                && fields.get("client_id").is_none_or(|value| value == &id) =>
        {
            (id, Some(secret), true)
        }
        Some(_) => return Err(OAuthError::invalid_client()),
        None => (
            fields
                .remove("client_id")
                .ok_or_else(OAuthError::invalid_client)?,
            fields.remove("client_secret"),
            false,
        ),
    };
    let client = client::lookup(control, &id).await?;
    client::authenticate(&client, secret.as_deref(), used_basic)?;
    Ok((fields, client))
}

pub(super) async fn retire_lease(host: Arc<Host>, id: Uuid) -> Result<(), OAuthError> {
    tokio::task::spawn_blocking(move || host.content.grants.revoke(id))
        .await
        .map_err(|_| OAuthError::busy())?
        .map_err(|_| OAuthError::busy())?;
    Ok(())
}
