mod controls;
mod personal;
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
pub(super) use controls::{controlled, routes};
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
        Ok(boundary) => {
            let boundary = match &state.auth.control {
                Some(control) => boundary.with_resolver(Arc::new(personal::Resolver {
                    control: control.clone(),
                    host: state.host.clone(),
                    mcp_resource: state
                        .oauth
                        .as_ref()
                        .map(|_| super::mcp_oauth::resource(state)),
                })),
                None => boundary,
            };
            lapis_content_http::router_with_boundary(boundary.clone()).merge(
                lapis_content_mcp::hosted_router(
                    boundary,
                    state.host.content.cancellation.clone(),
                    state.oauth.is_some(),
                )?,
            )
        }
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
    let oauth_mcp = request.uri().path() == "/mcp" && super::mcp_oauth::enabled(&state);
    let mut response = next.run(request).await;
    if oauth_mcp && response.status() == StatusCode::UNAUTHORIZED {
        let scope = super::mcp_oauth::resource_scopes(&state).join(" ");
        let challenge = format!(
            "Bearer resource_metadata=\"{}/.well-known/oauth-protected-resource/mcp\", scope=\"{scope}\"",
            state.origin,
        );
        if let Ok(value) = challenge.parse() {
            response.headers_mut().insert("www-authenticate", value);
        }
    }
    response
}

async fn status(state: State) -> Result<ContentHostStatus, Failure> {
    let available = !state.host.content.closed() && boundary(&state.host, &state.origin).is_ok();
    let host = state.host.clone();
    let access = blocking(move || host.content.grants.status().map_err(native)).await?;
    let personal_tokens = match &state.auth.control {
        Some(control) => match control
            .personal_tokens(crate::control::now_ms().map_err(super::browser::store)?)
            .await
        {
            Ok(tokens) => tokens,
            Err(_) if !access.enabled => Vec::new(),
            Err(error) => return Err(super::browser::store(error)),
        },
        None => Vec::new(),
    };
    let mcp_connections = match &state.auth.control {
        Some(control) if state.oauth.is_some() => match control
            .connected_mcp(crate::control::now_ms().map_err(super::browser::store)?)
            .await
        {
            Ok(connections) => connections,
            Err(_) if !access.enabled => Vec::new(),
            Err(error) => return Err(super::browser::store(error)),
        },
        _ => Vec::new(),
    };
    Ok(ContentHostStatus {
        mcp_connections,
        personal_tokens,
        operation_permissions: state.host.content.grants.operation_permissions(),
        access,
        available,
        persistent: state.auth.control.is_some(),
        api_url: available.then(|| format!("{}/api/content/v1", state.origin)),
        mcp_url: available.then(|| format!("{}/mcp", state.origin)),
    })
}
