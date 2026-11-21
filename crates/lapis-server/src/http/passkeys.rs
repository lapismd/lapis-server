use super::RegisterRoute as _;
use super::{State, auth, browser, errors::Failure, routes::body};
use crate::control::{Control, Session, now_ms, passkeys::PasskeySummary};
use axum::{
    Json, Router,
    extract::{ConnectInfo, Extension, Path, State as AxumState, rejection::JsonRejection},
    http::{HeaderMap, StatusCode},
    response::Response,
};
use lapis_app_protocol::ErrorCode;
use lapis_app_protocol::{api_route, http_docs::json_body};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    net::SocketAddr,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use uuid::Uuid;
use webauthn_rs::prelude::*;

pub(super) struct Passkeys {
    webauthn: Webauthn,
    pending: Mutex<BTreeMap<Uuid, (Instant, Ceremony)>>,
    finish: Mutex<()>,
}

enum Ceremony {
    Register(Uuid, String, PasskeyRegistration),
    Authenticate(String, Option<Uuid>, PasskeyAuthentication),
}

#[derive(Serialize)]
struct Challenge<T> {
    id: Uuid,
    options: T,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Label {
    label: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Finish<T> {
    id: Uuid,
    credential: T,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Authenticate {
    username: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Password {
    password: String,
}

impl Passkeys {
    pub fn new(origin: &str) -> Option<Self> {
        let url = url::Url::parse(origin).ok()?;
        let host = url.host_str()?;
        if url.scheme() != "https" && !(url.scheme() == "http" && host == "localhost") {
            return None;
        }
        let webauthn = WebauthnBuilder::new(host, &url)
            .ok()?
            .rp_name("Lapis Server")
            .build()
            .ok()?;
        Some(Self {
            webauthn,
            pending: Mutex::new(BTreeMap::new()),
            finish: Mutex::new(()),
        })
    }

    async fn challenge<T>(
        &self,
        options: T,
        ceremony: Ceremony,
    ) -> Result<Json<Challenge<T>>, Failure> {
        let mut pending = self.pending.lock().await;
        let now = Instant::now();
        pending.retain(|_, (deadline, _)| now < *deadline);
        if pending.len() >= 128 {
            return Err(Failure::new(
                ErrorCode::Busy,
                "Too many pending passkey requests",
            ));
        }
        let id = Uuid::new_v4();
        pending.insert(id, (now + Duration::from_secs(300), ceremony));
        Ok(Json(Challenge { id, options }))
    }

    async fn consume(&self, id: Uuid) -> Result<Ceremony, Failure> {
        self.pending
            .lock()
            .await
            .remove(&id)
            .filter(|(deadline, _)| Instant::now() < *deadline)
            .map(|(_, ceremony)| ceremony)
            .ok_or_else(denied)
    }
}

pub(super) fn owner_routes() -> Router<State> {
    Router::new()
        .api(api_route!("/api/v3/auth/passkeys", Browser, { get(list) => ("List the owner’s enrolled passkeys", 200, None, json_body::<serde_json::Value>()) }))
        .api(api_route!("/api/v3/auth/passkeys/{id}", Browser, { patch(rename) => ("Rename a passkey", 204, json_body::<serde_json::Value>(), None), delete(remove) => ("Remove a passkey", 204, None, None) }))
        .api(api_route!("/api/v3/auth/passkeys/register/options", Browser, { post(register) => ("Start passkey enrollment", 200, json_body::<serde_json::Value>(), json_body::<serde_json::Value>()) }))
        .api(api_route!("/api/v3/auth/passkeys/register/verify", Browser, { post(verify_registration) => ("Verify and save a new passkey", 204, json_body::<serde_json::Value>(), None) }))
        .api(api_route!("/api/v3/auth/passkeys/reauthenticate/options", Browser, { post(reauth_options) => ("Start passkey reauthentication", 200, None, json_body::<serde_json::Value>()) }))
        .api(api_route!("/api/v3/auth/passkeys/reauthenticate/verify", Browser, { post(reauth_verify) => ("Verify a passkey for recent owner authentication", 204, json_body::<serde_json::Value>(), None) }))
        .api(api_route!("/api/v3/auth/reauthenticate", Browser, { post(reauth_password) => ("Verify the owner password for recent authentication", 204, json_body::<serde_json::Value>(), None) }))
}

pub(super) fn public_routes() -> Router<State> {
    Router::new()
        .api(api_route!("/api/v3/auth/passkeys/login/options", Login, { post(login_options) => ("Start passkey sign-in", 200, json_body::<serde_json::Value>(), json_body::<serde_json::Value>()) }))
        .api(api_route!("/api/v3/auth/passkeys/login/verify", Login, { post(login_verify) => ("Verify passkey sign-in", 200, json_body::<serde_json::Value>(), json_body::<lapis_app_protocol::browser::SessionInfo>()) }))
        .layer(axum::extract::DefaultBodyLimit::max(32 * 1024))
}

fn service(state: &State) -> Result<(&Control, &Passkeys), Failure> {
    match (&state.auth.control, &state.passkeys) {
        (Some(control), Some(passkeys)) => Ok((control, passkeys)),
        _ => Err(Failure::new(
            ErrorCode::Unsupported,
            "Passkeys are unavailable for this server address",
        )),
    }
}

fn label(input: Label) -> Result<String, Failure> {
    let label = input.label.trim();
    if label.is_empty() || label.len() > 100 || input.label.chars().any(char::is_control) {
        return Err(Failure::new(
            ErrorCode::InvalidRequest,
            "Enter a passkey name between 1 and 100 bytes",
        ));
    }
    Ok(label.into())
}

fn denied() -> Failure {
    Failure(
        StatusCode::UNAUTHORIZED,
        crate::host::error(
            ErrorCode::PermissionDenied,
            "Passkey verification declined or expired",
        ),
    )
}

fn recent_required() -> Failure {
    Failure::new(
        ErrorCode::PermissionDenied,
        "Verify your identity before changing passkeys",
    )
}

async fn login_options(
    AxumState(state): AxumState<State>,
    headers: HeaderMap,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    input: Result<Json<Authenticate>, JsonRejection>,
) -> Result<Json<Challenge<RequestChallengeResponse>>, Failure> {
    auth::origin(&headers, &state.origin, true)?;
    auth::version(&headers, false)?;
    let (control, _) = service(&state)?;
    let now = now_ms().map_err(browser::store)?;
    let peer = peer_id(peer);
    allowed(control, &peer, now).await?;
    let username = body(input)?.username;
    if username.len() > 800
        || !control
            .owner_name()
            .await
            .map_err(browser::store)?
            .is_some_and(|owner| owner.eq_ignore_ascii_case(&username))
    {
        control
            .passkey_login_failed(&peer, now)
            .await
            .map_err(browser::store)?;
        return Err(denied());
    }
    authenticate_options(&state, peer, None).await
}

async fn reauth_options(
    AxumState(state): AxumState<State>,
    session: Option<Extension<Session>>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
) -> Result<Json<Challenge<RequestChallengeResponse>>, Failure> {
    let Extension(session) = session.ok_or_else(Failure::unauthorized)?;
    authenticate_options(&state, peer_id(peer), Some(session.id)).await
}

async fn authenticate_options(
    state: &State,
    peer: String,
    session: Option<Uuid>,
) -> Result<Json<Challenge<RequestChallengeResponse>>, Failure> {
    let (control, passkeys) = service(state)?;
    allowed(control, &peer, now_ms().map_err(browser::store)?).await?;
    let keys: Vec<_> = control
        .passkeys(&state.origin)
        .await
        .map_err(browser::store)?
        .into_iter()
        .map(|record| record.key)
        .collect();
    let (options, authentication) = passkeys
        .webauthn
        .start_passkey_authentication(&keys)
        .map_err(|_| denied())?;
    passkeys
        .challenge(
            options,
            Ceremony::Authenticate(peer, session, authentication),
        )
        .await
}

async fn login_verify(
    AxumState(state): AxumState<State>,
    headers: HeaderMap,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    input: Result<Json<Finish<PublicKeyCredential>>, JsonRejection>,
) -> Result<Response, Failure> {
    auth::origin(&headers, &state.origin, true)?;
    auth::version(&headers, false)?;
    let (control, passkeys) = service(&state)?;
    let _finish = passkeys.finish.lock().await;
    let now = now_ms().map_err(browser::store)?;
    verify_assertion(&state, peer_id(peer), None, body(input)?, now).await?;
    browser::issued_response(&state, control.issue(now).await.map_err(browser::store)?)
}

async fn reauth_verify(
    AxumState(state): AxumState<State>,
    session: Option<Extension<Session>>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    input: Result<Json<Finish<PublicKeyCredential>>, JsonRejection>,
) -> Result<StatusCode, Failure> {
    let Extension(session) = session.ok_or_else(Failure::unauthorized)?;
    let (control, passkeys) = service(&state)?;
    let _finish = passkeys.finish.lock().await;
    let now = now_ms().map_err(browser::store)?;
    verify_assertion(&state, peer_id(peer), Some(session.id), body(input)?, now).await?;
    mark_verified(control, session.id, now).await
}

async fn reauth_password(
    AxumState(state): AxumState<State>,
    session: Option<Extension<Session>>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    input: Result<Json<Password>, JsonRejection>,
) -> Result<StatusCode, Failure> {
    let Extension(session) = session.ok_or_else(Failure::unauthorized)?;
    let (control, _) = service(&state)?;
    let now = now_ms().map_err(browser::store)?;
    control
        .verify_password(
            &session.username,
            &body(input)?.password,
            &peer_id(peer),
            now,
        )
        .await
        .map_err(browser::login_failure)?;
    mark_verified(control, session.id, now).await
}

mod assertion;
mod enrollment;
use assertion::{allowed, mark_verified, peer_id, verify_assertion};
use enrollment::{list, register, remove, rename, verify_registration};
