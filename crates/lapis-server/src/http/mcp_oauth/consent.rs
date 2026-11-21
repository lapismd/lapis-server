use super::*;
use crate::control::{
    Session,
    mcp_oauth::{Client, Consent},
    now_ms, passwords,
};
use axum::{
    extract::{Extension, Path, RawQuery, rejection::JsonRejection},
    response::Redirect,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, time::Instant};
use url::Url;

struct RequestAccess {
    client: Client,
    callback: String,
    state: Option<String>,
    challenge: String,
    scopes: BTreeSet<String>,
    generation: u64,
}
pub(super) struct Code {
    pub client: String,
    pub redirect: String,
    pub challenge: String,
    pub generation: u64,
    pub grant: Consent,
}
#[derive(Default)]
pub(super) struct Pending {
    requests: BTreeMap<Uuid, (Instant, RequestAccess)>,
    pub codes: BTreeMap<String, (Instant, Code)>,
}
impl Pending {
    fn prune(&mut self, now: Instant) {
        self.requests.retain(|_, (deadline, _)| now < *deadline);
        self.codes.retain(|_, (deadline, _)| now < *deadline);
    }
    fn len(&self) -> usize {
        self.requests.len() + self.codes.len()
    }
}

pub(super) async fn authorize(
    AxumState(state): AxumState<State>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> Result<Redirect, OAuthError> {
    let (control, oauth) = resource_ready(&state, &headers)?;
    let permit = state
        .host
        .content
        .oauth_admission
        .clone()
        .try_acquire_owned()
        .map_err(|_| OAuthError::busy())?;
    let fields = params(query.as_deref().unwrap_or("").as_bytes())?;
    let field = |name: &str| {
        fields
            .get(name)
            .map(String::as_str)
            .ok_or_else(OAuthError::invalid_request)
    };
    let client = client::lookup(control, field("client_id")?).await?;
    let callback = field("redirect_uri")?;
    if !client::callback_allowed(&client, callback) {
        return Err(OAuthError::invalid_request());
    }
    let request_state = fields.get("state").cloned();
    let error = |code| {
        redirect(
            callback,
            &state.origin,
            request_state.as_deref(),
            "error",
            code,
        )
    };
    if field("response_type") != Ok("code") {
        return error("unsupported_response_type");
    }
    if field("resource").ok() != Some(resource(&state).as_str()) {
        return error("invalid_target");
    }
    let challenge = fields.get("code_challenge").cloned().unwrap_or_default();
    if field("code_challenge_method") != Ok("S256")
        || !URL_SAFE_NO_PAD
            .decode(&challenge)
            .is_ok_and(|bytes| bytes.len() == 32 && URL_SAFE_NO_PAD.encode(bytes) == challenge)
    {
        return error("invalid_request");
    }
    let mut scopes: BTreeSet<String> = fields
        .get("scope")
        .into_iter()
        .flat_map(|v| v.split_ascii_whitespace())
        .map(str::to_owned)
        .collect();
    scopes.insert("mcp:read".into());
    let available = state.host.content.grants.operation_permissions();
    if scopes.iter().any(|scope| {
        !matches!(scope.as_str(), "mcp:read" | "offline_access") && !available.contains(scope)
    }) {
        return error("invalid_scope");
    }
    drop(permit);
    let mut pending = oauth.pending.lock().await;
    let now = Instant::now();
    pending.prune(now);
    if pending.len() >= 128 {
        return error("temporarily_unavailable");
    }
    let id = Uuid::new_v4();
    pending.requests.insert(
        id,
        (
            now + Duration::from_secs(300),
            RequestAccess {
                client,
                callback: callback.into(),
                state: request_state,
                challenge,
                scopes,
                generation: state.host.content.generation(),
            },
        ),
    );
    Ok(Redirect::to(&format!("/?mcp-consent={id}")))
}

#[derive(Serialize)]
pub(super) struct Details {
    client_name: String,
    client_id: String,
    callback_host: String,
    permissions: Vec<String>,
}
pub(super) async fn inspect(
    AxumState(state): AxumState<State>,
    session: Option<Extension<Session>>,
    Path(id): Path<Uuid>,
) -> Result<Json<Details>, Failure> {
    let Extension(_owner) = session.ok_or_else(Failure::unauthorized)?;
    let oauth = state
        .oauth
        .as_ref()
        .filter(|_| enabled(&state))
        .ok_or_else(OAuthError::not_found)?;
    let mut pending = oauth.pending.lock().await;
    pending.prune(Instant::now());
    let (_, entry) = pending
        .requests
        .get(&id)
        .filter(|(_, entry)| entry.generation == state.host.content.generation())
        .ok_or_else(OAuthError::invalid_grant)?;
    let url = Url::parse(&entry.callback).map_err(|_| OAuthError::invalid_request())?;
    Ok(Json(Details {
        client_name: entry.client.name.clone(),
        client_id: entry.client.id.clone(),
        callback_host: url.host_str().unwrap_or_default().into(),
        permissions: entry
            .scopes
            .iter()
            .filter(|scope| !matches!(scope.as_str(), "mcp:read" | "offline_access"))
            .cloned()
            .collect(),
    }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Decision {
    approve: bool,
    #[serde(default)]
    vault_ids: Vec<Uuid>,
    #[serde(default)]
    permissions: Vec<String>,
}
pub(super) async fn decide(
    AxumState(state): AxumState<State>,
    session: Option<Extension<Session>>,
    Path(id): Path<Uuid>,
    body: Result<Json<Decision>, JsonRejection>,
) -> Result<Json<Value>, Failure> {
    let Extension(session) = session.ok_or_else(Failure::unauthorized)?;
    let decision = super::super::routes::body(body)?;
    let result = super::super::content::controlled(state, move |state| async move {
        let oauth = state.oauth.as_ref().filter(|_| enabled(&state)).ok_or_else(OAuthError::not_found)?;
        let control = state.auth.control.as_ref().ok_or_else(Failure::unauthorized)?;
        let now = now_ms().map_err(OAuthError::store)?;
        if !control.active_session(session.id, now).await.map_err(OAuthError::store)? {
            return Err(Failure::unauthorized());
        }
        let mut pending = oauth.pending.lock().await;
        let now = Instant::now();
        pending.prune(now);
        let (_, entry) = pending.requests.remove(&id).filter(|(_, entry)| entry.generation == state.host.content.generation()).ok_or_else(OAuthError::invalid_grant)?;
        if !decision.approve {
            return Ok(Json(json!({"redirect": callback(&entry.callback, &state.origin, entry.state.as_deref(), "error", "access_denied")? })));
        }
        if decision.vault_ids.is_empty() || decision.vault_ids.len() > 20
            || decision.vault_ids.iter().collect::<BTreeSet<_>>().len() != decision.vault_ids.len()
            || decision.permissions.iter().collect::<BTreeSet<_>>().len() != decision.permissions.len()
            || decision.permissions.iter().any(|p| !entry.scopes.contains(p) || matches!(p.as_str(), "mcp:read" | "offline_access"))
        {
            return Err(OAuthError::invalid_request().into());
        }
        let operations = state.host.content.operations.snapshot(&decision.permissions).map_err(crate::host::content::native)?;
        let host = state.host.clone();
        let vault_ids = decision.vault_ids.clone();
        super::super::errors::blocking(move || {
            let _guard = host.content.admission()?;
            host.content_vaults(&vault_ids)?;
            Ok(())
        }).await?;
        let generation = state.host.content.generation();
        let code = passwords::token().map_err(OAuthError::store)?;
        let target = callback(&entry.callback, &state.origin, entry.state.as_deref(), "code", &code)?;
        let mut scopes = vec!["mcp:read".to_owned()];
        if entry.scopes.contains("offline_access") { scopes.push("offline_access".into()); }
        scopes.extend(decision.permissions.clone());
        pending.codes.insert(passwords::digest(&code), (now + Duration::from_secs(300), Code {
            client: entry.client.id.clone(), redirect: entry.callback, challenge: entry.challenge, generation,
            grant: Consent {
                client_id: entry.client.id, resource: resource(&state), label: entry.client.name,
                vault_ids: decision.vault_ids, permissions: decision.permissions, operations, scope: scopes.join(" "),
            },
        }));
        Ok(Json(json!({"redirect": target})))
    }).await?;
    Ok(result)
}

fn callback(
    uri: &str,
    issuer: &str,
    state: Option<&str>,
    key: &str,
    value: &str,
) -> Result<String, OAuthError> {
    let mut redirect = Url::parse(uri).map_err(|_| OAuthError::invalid_request())?;
    let mut query = redirect.query_pairs_mut();
    query.append_pair(key, value).append_pair("iss", issuer);
    if let Some(state) = state {
        query.append_pair("state", state);
    }
    drop(query);
    Ok(redirect.into())
}
fn redirect(
    uri: &str,
    issuer: &str,
    state: Option<&str>,
    key: &str,
    value: &str,
) -> Result<Redirect, OAuthError> {
    Ok(Redirect::to(&callback(uri, issuer, state, key, value)?))
}

pub(super) async fn connections(
    AxumState(state): AxumState<State>,
    session: Option<Extension<Session>>,
) -> Result<Json<Vec<lapis_app_protocol::content::McpConnectionSummary>>, Failure> {
    let Extension(_owner) = session.ok_or_else(Failure::unauthorized)?;
    let control = state
        .auth
        .control
        .as_ref()
        .ok_or_else(OAuthError::not_found)?;
    Ok(Json(
        control
            .connected_mcp(now_ms().map_err(OAuthError::store)?)
            .await
            .map_err(OAuthError::store)?,
    ))
}

pub(super) async fn disconnect(
    AxumState(state): AxumState<State>,
    session: Option<Extension<Session>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, Failure> {
    let Extension(_owner) = session.ok_or_else(Failure::unauthorized)?;
    super::super::content::controlled(state, move |state| async move {
        let control = state
            .auth
            .control
            .as_ref()
            .ok_or_else(OAuthError::not_found)?;
        control
            .disconnect_mcp(id, now_ms().map_err(OAuthError::store)?)
            .await
            .map_err(OAuthError::store)?;
        token::retire_lease(state.host.clone(), id).await?;
        Ok(StatusCode::NO_CONTENT)
    })
    .await
}
