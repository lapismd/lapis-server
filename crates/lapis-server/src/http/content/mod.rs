mod controls;
use super::{
    State,
    errors::{Failure, blocking},
};
use crate::{
    control::Control,
    host::{Host, content::native},
};
use axum::{
    Router,
    body::Body,
    extract::State as AxumState,
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
};
pub(super) use controls::routes;
use lapis_app_protocol::content::ContentHostStatus;
use lapis_content_http::Boundary;
use std::{io, sync::Arc};

fn boundary(host: &Host, origin: &str) -> io::Result<Boundary> {
    Boundary::new(host.content.grants.clone(), origin)
}

/// Startup only, before exposing the router. Unsafe origins never restore access.
/// The durable preference contains no credential material.
pub async fn restore_settings(
    host: Arc<Host>,
    control: Arc<Control>,
    origin: &str,
) -> io::Result<()> {
    let enabled = control.content_enabled().await? && boundary(&host, origin).is_ok();
    blocking(move || host.content.grants.set_enabled(enabled).map_err(native))
        .await
        .map_err(|_| io::Error::other("Content settings are unavailable"))
}

pub(super) fn transport(state: &State) -> io::Result<Router<State>> {
    let routes = match boundary(&state.host, &state.origin) {
        Ok(boundary) => lapis_content_http::router_with_boundary(boundary.clone()).merge(
            lapis_content_mcp::router_with_boundary(
                boundary,
                state.host.content.cancellation.clone(),
            )?,
        ),
        Err(_) => Router::new()
            .route(
                "/mcp",
                axum::routing::any(|| async { StatusCode::NOT_FOUND }),
            )
            .route(
                "/api/content/v1",
                axum::routing::any(|| async { StatusCode::NOT_FOUND }),
            )
            .route(
                "/api/content/v1/{*path}",
                axum::routing::any(|| async { StatusCode::NOT_FOUND }),
            ),
    };
    Ok(routes
        .layer(middleware::from_fn_with_state(state.clone(), enabled))
        .with_state(()))
}

async fn enabled(
    AxumState(state): AxumState<State>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if !state.host.ready()
        || state.host.content.closed()
        || !state.host.content.grants.is_enabled().unwrap_or(false)
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    next.run(request).await
}

async fn status(state: State) -> Result<ContentHostStatus, Failure> {
    let available = !state.host.content.closed() && boundary(&state.host, &state.origin).is_ok();
    let host = state.host.clone();
    let access = blocking(move || host.content.grants.status().map_err(native)).await?;
    Ok(ContentHostStatus {
        access,
        available,
        persistent: state.auth.control.is_some(),
        api_url: available.then(|| format!("{}/api/content/v1", state.origin)),
        mcp_url: available.then(|| format!("{}/mcp", state.origin)),
    })
}
