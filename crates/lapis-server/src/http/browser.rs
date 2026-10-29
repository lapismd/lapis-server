use super::{
    State, auth, cookies,
    errors::{Failure, blocking},
};
use crate::{
    control::{LoginError, Session, now_ms},
    host::Principal,
};
use axum::{
    Json,
    body::to_bytes,
    extract::{ConnectInfo, Request, State as AxumState},
    http::{HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use lapis_app_protocol::{
    ErrorCode, PROTOCOL_MAJOR,
    browser::{HostInfo, LoginRequest, LogoutReply, SessionInfo},
};
use std::net::SocketAddr;

pub(super) async fn info(AxumState(state): AxumState<State>) -> Json<HostInfo> {
    Json(HostInfo {
        protocol_major: PROTOCOL_MAJOR,
        build_id: state.build_id,
        browser_auth: state.auth.control.is_some(),
        manage_vaults: state.host.manages_vaults(),
        transfer_vaults: state.host.manages_vaults(),
    })
}
pub(super) async fn login(
    AxumState(state): AxumState<State>,
    request: Request,
) -> Result<Response, Failure> {
    auth::origin(request.headers(), &state.origin, true)?;
    auth::version(request.headers(), false)?;
    let control = state.auth.control.as_ref().ok_or_else(|| {
        Failure::new(
            ErrorCode::Unsupported,
            "Browser authentication is unavailable",
        )
    })?;
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map_or_else(|| "local".into(), |peer| peer.0.ip().to_string());
    let bytes = to_bytes(request.into_body(), 16 * 1024)
        .await
        .map_err(|_| Failure::new(ErrorCode::InvalidRequest, "Invalid login request"))?;
    let input: LoginRequest = serde_json::from_slice(&bytes)
        .map_err(|_| Failure::new(ErrorCode::InvalidRequest, "Invalid login request"))?;
    let issued = control
        .login(
            &input.username,
            &input.password,
            &peer,
            now_ms().map_err(store)?,
        )
        .await
        .map_err(|error| match error {
            LoginError::Denied => Failure(
                StatusCode::UNAUTHORIZED,
                crate::host::error(ErrorCode::PermissionDenied, "Invalid username or password"),
            ),
            LoginError::Throttled => Failure(
                StatusCode::TOO_MANY_REQUESTS,
                crate::host::error(ErrorCode::Busy, "Login temporarily blocked"),
            ),
            LoginError::Store(_) => store(std::io::Error::other("Authentication store failed")),
        })?;
    let mut response = Json(session_info(&issued.session)).into_response();
    response.headers_mut().insert(
        "set-cookie",
        HeaderValue::from_str(&cookies::issue(&issued.token, state.auth.insecure_cookie))
            .map_err(|_| store(std::io::Error::other("Cookie failed")))?,
    );
    Ok(response)
}
pub(super) async fn current(request: Request) -> Result<Json<SessionInfo>, Failure> {
    Ok(Json(session_info(
        request
            .extensions()
            .get::<Session>()
            .ok_or_else(Failure::unauthorized)?,
    )))
}
pub(super) async fn logout(
    AxumState(state): AxumState<State>,
    request: Request,
) -> Result<Response, Failure> {
    let session = request
        .extensions()
        .get::<Session>()
        .ok_or_else(Failure::unauthorized)?;
    state
        .auth
        .control
        .as_ref()
        .ok_or_else(Failure::unauthorized)?
        .revoke(session.id, now_ms().map_err(store)?)
        .await
        .map_err(store)?;
    let principal = Principal::Session(session.id);
    blocking(move || state.host.detach_principal(principal)).await?;
    let mut response = Json(LogoutReply { ok: true }).into_response();
    response.headers_mut().insert(
        "set-cookie",
        cookies::clear(state.auth.insecure_cookie)
            .parse()
            .map_err(|_| store(std::io::Error::other("Cookie failed")))?,
    );
    Ok(response)
}
fn session_info(session: &Session) -> SessionInfo {
    SessionInfo {
        username: session.username.clone(),
        csrf_token: session.csrf_token.clone(),
        expires_at: session.expires_at,
    }
}
pub(super) fn store(_: std::io::Error) -> Failure {
    Failure::new(ErrorCode::Io, "Authentication store is unavailable")
}
