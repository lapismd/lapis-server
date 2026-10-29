use super::{State, VERSION_HEADER, browser, cookies, errors::Failure};
use crate::{control::now_ms, host::Principal};
use axum::{
    extract::{Request, State as AxumState},
    http::{HeaderMap, Method},
    middleware::Next,
    response::{IntoResponse, Response},
};
use lapis_app_protocol::ErrorCode;
use subtle::ConstantTimeEq;

pub(super) async fn authorize(
    AxumState(state): AxumState<State>,
    mut request: Request,
    next: Next,
) -> Response {
    match authenticate(&state, &mut request).await {
        Ok(()) => next.run(request).await,
        Err(error) => error.into_response(),
    }
}
async fn authenticate(state: &State, request: &mut Request) -> Result<(), Failure> {
    let (principal, session) = if let Some(presented) = request.headers().get("authorization") {
        let token = presented
            .to_str()
            .ok()
            .and_then(|value| value.strip_prefix("Bearer "));
        if !token.is_some_and(|token| state.auth.matches(token)) {
            return Err(Failure::unauthorized());
        }
        origin(request.headers(), &state.origin, false)?;
        version(request.headers(), false)?;
        (Principal::Token, None)
    } else {
        let control = state
            .auth
            .control
            .as_ref()
            .ok_or_else(Failure::unauthorized)?;
        let token = cookies::read(request.headers(), state.auth.insecure_cookie)
            .ok_or_else(Failure::unauthorized)?;
        let session = control
            .resolve(token, now_ms().map_err(browser::store)?)
            .await
            .map_err(browser::store)?
            .ok_or_else(Failure::unauthorized)?;
        let readonly = matches!(*request.method(), Method::GET | Method::HEAD);
        origin(request.headers(), &state.origin, !readonly)?;
        let byte_route = request
            .uri()
            .path()
            .strip_prefix("/api/v3/clients/")
            .and_then(|tail| tail.split_once('/'))
            .is_some_and(|(_, route)| {
                route.starts_with("files/") || route.starts_with("resources/")
            });
        let export_route = request
            .uri()
            .path()
            .strip_prefix("/api/v3/vaults/")
            .and_then(|tail| tail.strip_suffix("/export"))
            .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok());
        let plugin_route = request
            .uri()
            .path()
            .strip_prefix("/__lapis/plugins/")
            .and_then(|tail| tail.split_once('/'))
            .is_some_and(|(id, tail)| {
                uuid::Uuid::parse_str(id).is_ok() && tail.split('/').count() >= 4
            });
        version(
            request.headers(),
            readonly && (byte_route || export_route || plugin_route),
        )?;
        if !readonly {
            let csrf = request
                .headers()
                .get("x-lapis-csrf")
                .map_or(&[][..], |value| value.as_bytes());
            if !bool::from(csrf.ct_eq(session.csrf_token.as_bytes())) {
                return Err(Failure::new(
                    ErrorCode::PermissionDenied,
                    "Invalid CSRF token",
                ));
            }
        }
        (Principal::Session(session.id), Some(session))
    };
    request.extensions_mut().insert(principal);
    if let Some(session) = session {
        request.extensions_mut().insert(session);
    }
    Ok(())
}
pub(super) async fn validate_stream(
    state: &State,
    principal: Principal,
    headers: &HeaderMap,
) -> Result<(), Failure> {
    let mut request = Request::new(axum::body::Body::empty());
    *request.headers_mut() = headers.clone();
    authenticate(state, &mut request).await?;
    if request.extensions().get::<Principal>() != Some(&principal) {
        return Err(Failure::unauthorized());
    }
    Ok(())
}
pub(super) fn origin(headers: &HeaderMap, allowed: &str, required: bool) -> Result<(), Failure> {
    if headers
        .get("origin")
        .is_some_and(|value| value.as_bytes() != allowed.as_bytes())
        || (required && !headers.contains_key("origin"))
    {
        return Err(Failure::new(
            ErrorCode::PermissionDenied,
            "Origin is not admitted",
        ));
    }
    Ok(())
}
pub(super) fn version(headers: &HeaderMap, allow_missing: bool) -> Result<(), Failure> {
    if headers
        .get(VERSION_HEADER)
        .map_or(!allow_missing, |value| value.as_bytes() != b"3")
    {
        return Err(Failure::new(
            ErrorCode::ProtocolMismatch,
            "Reload the matching application client",
        ));
    }
    Ok(())
}
pub(super) async fn response_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .entry("cache-control")
        .or_insert("no-store".parse().unwrap());
    response
        .headers_mut()
        .insert("x-content-type-options", "nosniff".parse().unwrap());
    response
}
