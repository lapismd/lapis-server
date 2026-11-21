mod document;

use super::{RegisterRoute as _, State, auth, browser, cookies, errors::Failure};
use axum::{
    Json, Router,
    extract::{Path, State as AxumState},
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Response},
};
use lapis_app_protocol::{api_route, http_docs::text};
use serde_json::Value;
use std::sync::{Arc, OnceLock};

pub use document::openapi_document;

const UI: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Lapis Server API</title>
  <link rel="stylesheet" href="/api/docs/assets/swagger-ui.css">
</head>
<body>
  <div id="swagger-ui"></div>
  <script src="/api/docs/assets/swagger-ui-bundle.js"></script>
  <script src="/api/docs/assets/lapis.js"></script>
</body>
</html>"#;

const INITIALIZER: &str = r##"
const headers = { "x-lapis-protocol": "3" };
SwaggerUIBundle({
  dom_id: "#swagger-ui",
  url: "/api/openapi.json",
  validatorUrl: null,
  persistAuthorization: false,
  requestInterceptor: async (request) => {
    const target = new URL(request.url, window.location.origin);
    if (target.origin !== window.location.origin) throw new Error("Use the Lapis Server origin");
    if (target.pathname.startsWith("/api/v3/")) {
      Object.assign(request.headers, headers);
      if (!["GET", "HEAD"].includes(request.method)) {
        const response = await fetch("/api/v3/auth/current", { headers, cache: "no-store", credentials: "same-origin" });
        if (!response.ok) throw new Error("Sign into Lapis Server first");
        request.headers["x-lapis-csrf"] = (await response.json()).csrf_token;
      }
    }
    if (target.pathname.startsWith("/api/content/v1/"))
      request.headers["x-lapis-content-api"] = "1";
    request.credentials = "same-origin";
    return request;
  }
});
"##;

pub(super) fn routes() -> Router<State> {
    Router::new()
        .api(api_route!("/api/docs", BrowserNavigation, {
            get(ui) => ("Open Swagger UI", 200, None, text("text/html"))
        }))
        .api(api_route!("/api/docs/assets/{asset}", BrowserNavigation, {
            get(asset) => ("Load a bundled Swagger UI asset", 200, None, text("application/octet-stream"))
        }))
        .api(api_route!("/api/openapi.json", BrowserNavigation, {
            get(json) => ("Download the generated OpenAPI document", 200, None, lapis_app_protocol::http_docs::json_body::<Value>())
        }))
}

async fn owner(state: &State, headers: &HeaderMap) -> Result<(), Failure> {
    auth::origin(headers, &state.origin, false)?;
    let control = state
        .auth
        .control
        .as_ref()
        .ok_or_else(Failure::unauthorized)?;
    let bearer =
        cookies::read(headers, state.auth.insecure_cookie).ok_or_else(Failure::unauthorized)?;
    control
        .resolve(bearer, crate::control::now_ms().map_err(browser::store)?)
        .await
        .map_err(browser::store)?
        .ok_or_else(Failure::unauthorized)?;
    Ok(())
}

async fn ui(AxumState(state): AxumState<State>, headers: HeaderMap) -> Result<Response, Failure> {
    owner(&state, &headers).await?;
    Ok(([
        (header::CONTENT_SECURITY_POLICY, "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'"),
        (header::REFERRER_POLICY, "no-referrer"),
    ], Html(UI)).into_response())
}

async fn json(
    AxumState(state): AxumState<State>,
    headers: HeaderMap,
) -> Result<Json<Value>, Failure> {
    owner(&state, &headers).await?;
    static SPEC: OnceLock<Value> = OnceLock::new();
    let mut document = SPEC.get_or_init(openapi_document).clone();
    let session_cookie = cookies::name(state.auth.insecure_cookie);
    document["components"]["securitySchemes"]["ownerSession"]["name"] =
        Value::String(session_cookie.into());
    Ok(Json(document))
}

async fn asset(
    AxumState(state): AxumState<State>,
    headers: HeaderMap,
    Path(asset): Path<String>,
) -> Result<Response, Failure> {
    owner(&state, &headers).await?;
    if asset == "lapis.js" {
        return Ok(([(header::CONTENT_TYPE, "text/javascript")], INITIALIZER).into_response());
    }
    if !matches!(asset.as_str(), "swagger-ui.css" | "swagger-ui-bundle.js") {
        return Ok(StatusCode::NOT_FOUND.into_response());
    }
    let file = utoipa_swagger_ui::serve(
        &asset,
        Arc::new(utoipa_swagger_ui::Config::from("/api/openapi.json")),
    )
    .ok()
    .flatten()
    .ok_or_else(|| {
        Failure::new(
            lapis_app_protocol::ErrorCode::Io,
            "Swagger UI asset is unavailable",
        )
    })?;
    let mut response = (
        [(header::CONTENT_TYPE, file.content_type)],
        file.bytes.into_owned(),
    )
        .into_response();
    if file.gzpipped {
        response
            .headers_mut()
            .insert(header::CONTENT_ENCODING, "gzip".parse().unwrap());
    }
    Ok(response)
}
