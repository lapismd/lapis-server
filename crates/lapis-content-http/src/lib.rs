//! Shared content HTTP transport. Host integration is intentionally explicit.
mod admission;
mod policy;
mod response;
mod routes;

use axum::{
    Router,
    extract::{Request, State},
    http::Method,
};
use lapis_app_core::services::content::ContentGrants;
use lapis_app_protocol::content::*;
use response::Failure;
pub use routes::READ_ROUTES;
use std::sync::Arc;
use tokio::sync::Semaphore;
use uuid::Uuid;

pub const API_HEADER: &str = "x-lapis-content-api";
pub const REQUEST_ID_HEADER: &str = "x-lapis-request-id";
pub const MAX_NATIVE_JOBS: usize = 64;

#[derive(Clone)]
struct HttpState {
    grants: Arc<ContentGrants>,
    policy: Arc<policy::Policy>,
    jobs: Arc<Semaphore>,
}

/// Mount only after native host consent/admission. The origin must be HTTPS or
/// HTTP loopback, matching the Host header delivered by the host's listener/proxy.
/// The host retains listener limits, token cleanup and shutdown/drain ownership.
pub fn router(grants: Arc<ContentGrants>, public_origin: &str) -> std::io::Result<Router> {
    let state = HttpState {
        grants,
        policy: Arc::new(policy::Policy::new(public_origin)?),
        jobs: Arc::new(Semaphore::new(MAX_NATIVE_JOBS)),
    };
    Ok(Router::new().nest(
        "/api/content/v1",
        Router::new().fallback(handle).with_state(state),
    ))
}

async fn handle(State(state): State<HttpState>, request: Request) -> axum::response::Response {
    let id = policy::correlation(request.headers()).unwrap_or_else(|_| Uuid::new_v4());
    let (parts, _body) = request.into_parts();
    let has_body = policy::has_body(&parts.headers);
    let result = admission::run(state.jobs.clone(), move || {
        state.policy.check(&parts)?;
        let bearer = policy::bearer(&parts.headers)?;
        state.grants.check_credential(bearer)?;
        policy::version(&parts.headers)?;
        policy::correlation(&parts.headers)?;
        if parts.method != Method::GET {
            return Err(Failure::method());
        }
        if has_body {
            return Err(Failure::invalid("Read requests cannot carry a body"));
        }
        let command = routes::command(&parts.uri)?;
        let request = ContentRequest {
            api_version: CONTENT_API_MAJOR,
            request_id: id,
            command,
        };
        Ok(state.grants.execute(bearer, request))
    })
    .await;
    let reply = match result {
        Ok(reply) => response::reply(reply),
        Err(failure) => failure.response(id),
    };
    response::decorate(reply, id, has_body)
}
