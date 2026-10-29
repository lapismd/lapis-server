use crate::http::State;
use axum::{
    Json,
    extract::{Request, State as AxumState},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};

pub(in crate::http) async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status":"ok"}))
}
pub(in crate::http) async fn ready(AxumState(state): AxumState<State>) -> Response {
    let ready = state.host.ready();
    (
        if ready {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        },
        Json(serde_json::json!({"status":if ready { "ready" } else { "unavailable" }})),
    )
        .into_response()
}
pub(in crate::http) async fn available(
    AxumState(state): AxumState<State>,
    request: Request,
    next: Next,
) -> Response {
    if !state.host.ready() && !matches!(request.uri().path(), "/healthz" | "/readyz") {
        return (StatusCode::SERVICE_UNAVAILABLE, "Server is shutting down").into_response();
    }
    next.run(request).await
}
