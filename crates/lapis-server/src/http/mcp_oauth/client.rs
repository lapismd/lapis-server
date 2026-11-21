use super::{OAuthError, resource_ready};
use crate::{
    control::{Control, mcp_oauth::Client, now_ms, passwords},
    http::State,
};
use axum::{
    Json,
    extract::{ConnectInfo, Request, State as AxumState},
    http::{HeaderMap, StatusCode},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::{Duration, Instant},
};
use subtle::ConstantTimeEq;
use url::Url;

#[derive(Deserialize, schemars::JsonSchema)]
pub(super) struct Metadata {
    #[serde(default)]
    client_id: Option<String>,
    client_name: String,
    redirect_uris: Vec<String>,
    #[serde(default)]
    token_endpoint_auth_method: Option<String>,
    #[serde(default)]
    token_endpoint_auth_methods_supported: Vec<String>,
    grant_types: Option<Vec<String>>,
    response_types: Option<Vec<String>>,
}

pub(super) async fn register(
    AxumState(state): AxumState<State>,
    request: Request,
) -> Result<(StatusCode, Json<Value>), OAuthError> {
    let (control, oauth) = resource_ready(&state, request.headers())?;
    let _permit = state
        .host
        .content
        .oauth_admission
        .clone()
        .try_acquire_owned()
        .map_err(|_| OAuthError::busy())?;
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map_or(IpAddr::V4(Ipv4Addr::LOCALHOST), |peer| peer.0.ip());
    {
        let now = Instant::now();
        let mut registrations = oauth.registrations.lock().await;
        registrations
            .retain(|_, (started, _)| now.duration_since(*started) < Duration::from_secs(60));
        if !registrations.contains_key(&peer) && registrations.len() >= 256 {
            return Err(OAuthError::busy());
        }
        let (_, count) = registrations.entry(peer).or_insert((now, 0));
        if *count >= 8 {
            return Err(OAuthError(
                StatusCode::TOO_MANY_REQUESTS,
                "temporarily_unavailable",
            ));
        }
        *count += 1;
    }
    let body = super::http_body(request, "application/json").await?;
    let meta: Metadata =
        serde_json::from_slice(&body).map_err(|_| OAuthError::invalid_client_metadata())?;
    if meta.client_id.is_some() {
        return Err(OAuthError::invalid_client_metadata());
    }
    let client = metadata(
        meta,
        format!("lmc_{}", passwords::token().map_err(OAuthError::store)?),
        false,
    )?;
    let mut client = client;
    let secret = if client.auth_method == "none" {
        None
    } else {
        let value = passwords::token().map_err(OAuthError::store)?;
        client.secret_verifier = Some(passwords::digest(&value));
        Some(value)
    };
    if !control
        .register_mcp_client(&client, now_ms().map_err(OAuthError::store)?)
        .await
        .map_err(OAuthError::store)?
    {
        return Err(OAuthError::busy());
    }
    let mut result = json!({
        "client_id": client.id, "client_name":client.name,"redirect_uris":client.redirects,
        "token_endpoint_auth_method":client.auth_method,"grant_types":["authorization_code","refresh_token"],"response_types":["code"]
    });
    if let Some(secret) = secret {
        result["client_secret"] = json!(secret);
        result["client_secret_expires_at"] = json!(0);
    }
    Ok((StatusCode::CREATED, Json(result)))
}

pub(super) async fn lookup(control: &Control, id: &str) -> Result<Client, OAuthError> {
    if id.starts_with("lmc_") && id.len() == 47 {
        return control
            .mcp_client(id)
            .await
            .map_err(OAuthError::store)?
            .ok_or_else(OAuthError::invalid_client);
    }
    let url = Url::parse(id).map_err(|_| OAuthError::invalid_client())?;
    if !approved_metadata_url(&url, id) {
        return Err(OAuthError::invalid_client());
    }
    let client = reqwest::Client::builder()
        .https_only(true)
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|_| OAuthError::invalid_client())?;
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|_| OAuthError::invalid_client())?;
    if !response.status().is_success() {
        return Err(OAuthError::invalid_client());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| OAuthError::invalid_client())?
    {
        if bytes.len() + chunk.len() > 16 * 1024 {
            return Err(OAuthError::invalid_client());
        }
        bytes.extend_from_slice(&chunk);
    }
    let meta: Metadata =
        serde_json::from_slice(&bytes).map_err(|_| OAuthError::invalid_client())?;
    if meta.client_id.as_deref() != Some(id) {
        return Err(OAuthError::invalid_client());
    }
    metadata(meta, id.into(), true).map_err(|_| OAuthError::invalid_client())
}

fn approved_metadata_url(url: &Url, original: &str) -> bool {
    // Fetch only the named providers' published documents. Other clients can
    // register directly, which never makes an outbound request.
    if original.len() > 2048
        || url.as_str() != original
        || url.scheme() != "https"
        || url.username() != ""
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return false;
    }
    match url.host_str() {
        Some("chatgpt.com") => {
            url.path().starts_with("/oauth/") && url.path().ends_with("/client.json")
        }
        Some("claude.ai" | "claude.com") => {
            url.path() == "/oauth/claude-code-client-metadata" || url.path().ends_with(".json")
        }
        _ => false,
    }
}

fn metadata(meta: Metadata, id: String, published: bool) -> Result<Client, OAuthError> {
    let method = if published
        && meta
            .token_endpoint_auth_methods_supported
            .iter()
            .any(|value| value == "none")
    {
        "none".into()
    } else {
        meta.token_endpoint_auth_method.unwrap_or_else(|| {
            if published {
                "none".into()
            } else {
                "client_secret_basic".into()
            }
        })
    };
    if meta.client_name.trim().is_empty()
        || meta.client_name.len() > 128
        || meta.client_name.chars().any(char::is_control)
        || meta.redirect_uris.is_empty()
        || meta.redirect_uris.len() > 10
        || !meta.redirect_uris.iter().all(|uri| valid_redirect(uri))
        || meta.grant_types.as_ref().is_some_and(|types| {
            types.is_empty()
                || types
                    .iter()
                    .any(|value| !matches!(value.as_str(), "authorization_code" | "refresh_token"))
        })
        || meta
            .response_types
            .as_ref()
            .is_some_and(|types| types.is_empty() || types.iter().any(|value| value != "code"))
        || (published && method != "none")
        || (!published
            && !matches!(
                method.as_str(),
                "none" | "client_secret_basic" | "client_secret_post"
            ))
    {
        return Err(OAuthError::invalid_client_metadata());
    }
    Ok(Client {
        id,
        name: meta.client_name,
        redirects: meta.redirect_uris,
        auth_method: method,
        secret_verifier: None,
    })
}

fn valid_redirect(value: &str) -> bool {
    let Ok(url) = Url::parse(value) else {
        return false;
    };
    let loopback = matches!(url.host_str(), Some("127.0.0.1" | "[::1]" | "localhost"));
    value.len() <= 2048
        && url.fragment().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && !url
            .query_pairs()
            .any(|(k, _)| matches!(k.as_ref(), "code" | "state" | "iss" | "error"))
        && url.host().is_some()
        && (url.scheme() == "https" || url.scheme() == "http" && loopback)
}

pub(super) fn callback_allowed(client: &Client, callback: &str) -> bool {
    if !valid_redirect(callback) {
        return false;
    }
    client.redirects.iter().any(|value| {
        value == callback || {
            let (Ok(mut registered), Ok(mut actual)) = (Url::parse(value), Url::parse(callback))
            else {
                return false;
            };
            if registered.scheme() != "http"
                || !matches!(
                    registered.host_str(),
                    Some("127.0.0.1" | "[::1]" | "localhost")
                )
            {
                return false;
            }
            registered.set_port(None).is_ok()
                && actual.set_port(None).is_ok()
                && registered == actual
        }
    })
}

pub(super) fn basic(headers: &HeaderMap) -> Result<Option<(String, String)>, OAuthError> {
    if headers.get_all("authorization").iter().count() > 1 {
        return Err(OAuthError::invalid_client());
    }
    let Some(raw) = headers.get("authorization") else {
        return Ok(None);
    };
    let (scheme, encoded) = raw
        .to_str()
        .ok()
        .and_then(|s| s.split_once(' '))
        .ok_or_else(OAuthError::invalid_client)?;
    if !scheme.eq_ignore_ascii_case("basic") || encoded.len() > 8192 {
        return Err(OAuthError::invalid_client());
    }
    let decoded = String::from_utf8(
        STANDARD
            .decode(encoded)
            .map_err(|_| OAuthError::invalid_client())?,
    )
    .map_err(|_| OAuthError::invalid_client())?;
    let (id, secret) = decoded
        .split_once(':')
        .ok_or_else(OAuthError::invalid_client)?;
    fn decode(value: &str) -> Option<String> {
        let encoded = format!("value={value}");
        let mut fields = url::form_urlencoded::parse(encoded.as_bytes());
        let (_, decoded) = fields.next()?;
        fields.next().is_none().then(|| decoded.into_owned())
    }
    Ok(Some((
        decode(id).ok_or_else(OAuthError::invalid_client)?,
        decode(secret).ok_or_else(OAuthError::invalid_client)?,
    )))
}

pub(super) fn authenticate(
    client: &Client,
    secret: Option<&str>,
    basic: bool,
) -> Result<(), OAuthError> {
    let valid = if client.auth_method == "none" {
        !basic && secret.is_none()
    } else {
        let correct_method = (basic && client.auth_method == "client_secret_basic")
            || (!basic && client.auth_method == "client_secret_post");
        correct_method
            && secret
                .zip(client.secret_verifier.as_deref())
                .is_some_and(|(value, verifier)| {
                    bool::from(
                        passwords::digest(value)
                            .as_bytes()
                            .ct_eq(verifier.as_bytes()),
                    )
                })
    };
    if valid {
        Ok(())
    } else {
        Err(OAuthError::invalid_client())
    }
}
