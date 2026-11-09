use crate::{MAX_HTTP_REQUESTS, MAX_REQUEST_BYTES, catalog::Catalog, handler::Handler};
use axum::{
    Router,
    extract::{Request, State},
    http::{HeaderValue, Uri},
    middleware::{self, Next},
    response::Response,
};
use lapis_app_protocol::content::ContentErrorCode;
use lapis_content_http::{Boundary, Rejection};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::never::NeverSessionManager,
};
use std::{sync::Arc, time::Duration};
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Clone)]
struct StateData {
    boundary: Boundary,
    requests: Arc<Semaphore>,
    cancellation: CancellationToken,
}
pub fn router(boundary: Boundary, cancellation: CancellationToken) -> std::io::Result<Router> {
    let handler = Handler {
        boundary: boundary.clone(),
        catalog: Arc::new(Catalog::new()?),
    };
    let uri: Uri = boundary
        .origin()
        .parse()
        .expect("boundary validates origin");
    let config = StreamableHttpServerConfig::default()
        .with_legacy_session_mode(false)
        .with_json_response(true)
        .with_sse_keep_alive(None)
        .with_sse_retry(None)
        .with_stateless_protocol_metadata_required(true)
        .with_max_request_body_bytes(MAX_REQUEST_BYTES)
        .with_allowed_hosts([uri.authority().unwrap().as_str().to_owned()])
        .with_allowed_origins([boundary.origin().to_owned()])
        .with_cancellation_token(cancellation.clone());
    let service = StreamableHttpService::new(
        move || Ok(handler.clone()),
        Arc::new(NeverSessionManager::default()),
        config,
    );
    let state = StateData {
        boundary,
        requests: Arc::new(Semaphore::new(MAX_HTTP_REQUESTS)),
        cancellation,
    };
    Ok(Router::new()
        .route_service("/mcp", service)
        .layer(middleware::from_fn_with_state(state, guard)))
}
async fn guard(State(state): State<StateData>, request: Request, next: Next) -> Response {
    let id = Uuid::new_v4();
    let work = async {
        let _permit = state.requests.clone().try_acquire_owned().map_err(|_| {
            Rejection::new(ContentErrorCode::Busy, "MCP request capacity exhausted")
        })?;
        let (parts, body) = request.into_parts();
        let parts = state.boundary.check(parts).await?;
        // Session IDs cannot select authority. This adapter has no sessions.
        if parts.headers.contains_key("mcp-session-id")
            || parts.headers.get_all("mcp-protocol-version").iter().count() > 1
        {
            return Err(Rejection::new(
                ContentErrorCode::InvalidRequest,
                "Invalid stateless MCP headers",
            ));
        }
        Ok(next.run(Request::from_parts(parts, body)).await)
    };
    let response = tokio::select! {
        biased;
        _ = state.cancellation.cancelled() => Err(Rejection::new(ContentErrorCode::Unavailable, "MCP transport stopped")),
        result = tokio::time::timeout(Duration::from_secs(15), work) => result
            .unwrap_or_else(|_| Err(Rejection::new(ContentErrorCode::Unavailable, "MCP request timed out"))),
    };
    let mut response = response.unwrap_or_else(|failure| failure.response(id));
    response
        .headers_mut()
        .insert("cache-control", HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    response
}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
