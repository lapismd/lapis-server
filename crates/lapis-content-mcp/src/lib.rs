//! Explicit shared MCP mount. Hosts own consent, grants, listeners and teardown.
mod catalog;
mod handler;
mod resources;
mod transport;

use lapis_app_core::services::content::ContentGrants;
use lapis_content_http::Boundary;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub const MAX_REQUEST_BYTES: usize = 64 * 1024;
pub const MAX_HTTP_REQUESTS: usize = 64;

/// Controlled test-grant adapter. Production OAuth and host consent are separate.
pub fn router(grants: Arc<ContentGrants>, origin: &str) -> std::io::Result<axum::Router> {
    router_with_boundary(Boundary::new(grants, origin)?, CancellationToken::new())
}

/// Share `boundary` with the content HTTP router to share native worker capacity.
/// Cancel transport, shut down/drain grants, then release host clients/listeners.
pub fn router_with_boundary(
    boundary: Boundary,
    cancellation: CancellationToken,
) -> std::io::Result<axum::Router> {
    transport::router(boundary, cancellation)
}
