use super::RegisterRoute as _;
use lapis_app_protocol::{
    api_route,
    http_docs::{form_body, json_body},
};
mod client;
mod consent;
mod token;

use super::{State, auth, errors::Failure};
use crate::control::Control;
use axum::{
    Json, Router,
    body::to_bytes,
    extract::{Request, State as AxumState},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io,
    net::IpAddr,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Default)]
pub(super) struct OAuth {
    pending: Mutex<consent::Pending>,
    registrations: Mutex<BTreeMap<IpAddr, (Instant, u8)>>,
}

pub(super) fn public_routes() -> Router<State> {
    Router::new()
        .api(api_route!("/.well-known/oauth-protected-resource", Public, { get(protected_resource) => ("Discover MCP resource authorization metadata", 200, None, json_body::<serde_json::Value>()) }))
        .api(api_route!("/.well-known/oauth-protected-resource/mcp", Public, { get(protected_resource) => ("Discover MCP resource authorization metadata", 200, None, json_body::<serde_json::Value>()) }))
        .api(api_route!("/.well-known/oauth-authorization-server", Public, { get(metadata) => ("Discover the MCP authorization server", 200, None, json_body::<serde_json::Value>()) }))
        .api(api_route!("/oauth/authorize", Public, { get(consent::authorize) => ("Start owner approval for an MCP OAuth client", 303, None, None) }))
        .api(api_route!("/oauth/register", Public, { post(client::register) => ("Register an OAuth client", 201, json_body::<client::Metadata>(), json_body::<serde_json::Value>()) }))
        .api(api_route!("/oauth/token", OAuthClient, { post(token::exchange) => ("Exchange an authorization code or rotate a refresh token", 200, form_body::<serde_json::Value>(), json_body::<crate::control::mcp_oauth::TokenPair>()) }))
        .api(api_route!("/oauth/revoke", OAuthClient, { post(token::revoke) => ("Revoke a token and its authorization", 200, form_body::<serde_json::Value>(), None) }))
}

pub(super) fn owner_routes() -> Router<State> {
    Router::new()
        .api(api_route!("/api/v3/content-access/mcp/consent/{id}", Browser, { get(consent::inspect) => ("Inspect the pending MCP application approval", 200, None, json_body::<serde_json::Value>()), post(consent::decide) => ("Approve or decline access to selected vaults", 200, json_body::<serde_json::Value>(), json_body::<serde_json::Value>()) }))
        .api(api_route!("/api/v3/content-access/mcp/connections", Browser, { get(consent::connections) => ("List authorized MCP applications", 200, None, json_body::<Vec<lapis_app_protocol::content::McpConnectionSummary>>()) }))
        .api(api_route!("/api/v3/content-access/mcp/connections/{id}", Browser, { delete(consent::disconnect) => ("Disconnect an authorized MCP application", 204, None, None) }))
}

pub(super) fn resource(state: &State) -> String {
    format!("{}/mcp", state.origin)
}

pub(super) fn enabled(state: &State) -> bool {
    state.oauth.is_some()
        && state.host.ready()
        && state.host.content.grants.is_enabled().unwrap_or(false)
}

fn resource_ready<'a>(
    state: &'a State,
    headers: &HeaderMap,
) -> Result<(&'a Control, &'a OAuth), OAuthError> {
    let (Some(control), Some(oauth)) = (&state.auth.control, &state.oauth) else {
        return Err(OAuthError::not_found());
    };
    let allowed_host = state.origin.strip_prefix("https://").unwrap_or("");
    if !enabled(state)
        || headers.get_all("host").iter().count() != 1
        || !headers
            .get("host")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|value| value.eq_ignore_ascii_case(allowed_host))
    {
        return Err(OAuthError::not_found());
    }
    auth::origin(headers, &state.origin, false).map_err(|_| OAuthError::invalid_request())?;
    Ok((control, oauth))
}

fn resource_scopes(state: &State) -> Vec<String> {
    let mut scopes = vec!["mcp:read".to_owned()];
    scopes.extend(state.host.content.grants.operation_permissions());
    scopes
}

pub(super) async fn icon() -> impl axum::response::IntoResponse {
    (
        [
            ("content-type", "image/png"),
            ("cache-control", "public, max-age=86400"),
            ("cross-origin-resource-policy", "cross-origin"),
            ("access-control-allow-origin", "*"),
        ],
        include_bytes!("../../../../web/public/icon.png").as_slice(),
    )
}

async fn protected_resource(
    AxumState(state): AxumState<State>,
    headers: HeaderMap,
) -> Result<Json<Value>, OAuthError> {
    resource_ready(&state, &headers)?;
    Ok(Json(json!({
        "resource":resource(&state),"authorization_servers":[state.origin],
        "bearer_methods_supported":["header"],"scopes_supported":resource_scopes(&state),"resource_name":"Lapis Server"
    })))
}

async fn metadata(
    AxumState(state): AxumState<State>,
    headers: HeaderMap,
) -> Result<Json<Value>, OAuthError> {
    resource_ready(&state, &headers)?;
    let mut scopes = resource_scopes(&state);
    scopes.insert(1, "offline_access".to_owned());
    Ok(Json(json!({
        "issuer":state.origin,
        "authorization_endpoint":format!("{}/oauth/authorize",state.origin),
        "token_endpoint":format!("{}/oauth/token",state.origin),
        "registration_endpoint":format!("{}/oauth/register",state.origin),
        "revocation_endpoint":format!("{}/oauth/revoke",state.origin),
        "response_types_supported":["code"],
        "response_modes_supported":["query"],
        "grant_types_supported":["authorization_code","refresh_token"],
        "code_challenge_methods_supported":["S256"],
        "scopes_supported":scopes,
        "token_endpoint_auth_methods_supported":["none","client_secret_basic","client_secret_post"],
        "revocation_endpoint_auth_methods_supported":["none","client_secret_basic","client_secret_post"],
        "authorization_response_iss_parameter_supported":true,
        "client_id_metadata_document_supported":true
    })))
}

async fn http_body(request: Request, media_type: &str) -> Result<axum::body::Bytes, OAuthError> {
    let provided = request
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .and_then(|text| text.split(';').next())
        .map(str::trim);
    if provided != Some(media_type) {
        return Err(OAuthError::invalid_request());
    }
    tokio::time::timeout(
        Duration::from_secs(10),
        to_bytes(request.into_body(), 16 * 1024),
    )
    .await
    .map_err(|_| OAuthError::invalid_request())?
    .map_err(|_| OAuthError::invalid_request())
}

fn params(value: &[u8]) -> Result<BTreeMap<String, String>, OAuthError> {
    if value.len() > 16 * 1024 {
        return Err(OAuthError::invalid_request());
    }
    let mut fields = BTreeMap::new();
    for (name, value) in url::form_urlencoded::parse(value) {
        if name.len() > 128
            || value.len() > 4096
            || fields.len() >= 32
            || fields
                .insert(name.into_owned(), value.into_owned())
                .is_some()
        {
            return Err(OAuthError::invalid_request());
        }
    }
    Ok(fields)
}

#[derive(Clone, Copy, PartialEq)]
struct OAuthError(StatusCode, &'static str);
impl OAuthError {
    fn invalid_request() -> Self {
        Self(StatusCode::BAD_REQUEST, "invalid_request")
    }
    fn invalid_grant() -> Self {
        Self(StatusCode::BAD_REQUEST, "invalid_grant")
    }
    fn invalid_client() -> Self {
        Self(StatusCode::UNAUTHORIZED, "invalid_client")
    }
    fn invalid_client_metadata() -> Self {
        Self(StatusCode::BAD_REQUEST, "invalid_client_metadata")
    }
    fn invalid_target() -> Self {
        Self(StatusCode::BAD_REQUEST, "invalid_target")
    }
    fn not_found() -> Self {
        Self(StatusCode::NOT_FOUND, "invalid_request")
    }
    fn busy() -> Self {
        Self(StatusCode::SERVICE_UNAVAILABLE, "temporarily_unavailable")
    }
    fn store(_: io::Error) -> Self {
        Self(StatusCode::SERVICE_UNAVAILABLE, "server_error")
    }
}
impl IntoResponse for OAuthError {
    fn into_response(self) -> Response {
        let mut response = (self.0, Json(json!({"error": self.1}))).into_response();
        if self.1 == "invalid_client" {
            response.headers_mut().insert(
                "www-authenticate",
                axum::http::HeaderValue::from_static("Basic realm=\"lapis-mcp\""),
            );
        }
        response
    }
}
impl From<OAuthError> for Failure {
    fn from(error: OAuthError) -> Self {
        Self(
            error.0,
            crate::host::error(
                lapis_app_protocol::ErrorCode::PermissionDenied,
                "MCP authorization is unavailable or expired",
            ),
        )
    }
}
