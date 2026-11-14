use crate::{Boundary, policy, response};
use axum::{
    Json, Router,
    extract::{Path, Query, Request, State},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use lapis_app_protocol::{content::*, operations::*};
use serde::Deserialize;
use uuid::Uuid;

pub fn routes() -> Router<Boundary> {
    Router::new().route("/operations", get(catalog)).route(
        "/vaults/{vault}/plugins/{plugin}/operations/{operation}",
        post(invoke),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    vault_id: Option<Uuid>,
}

async fn catalog(State(boundary): State<Boundary>, request: Request) -> Response {
    let (parts, _) = request.into_parts();
    let id = policy::correlation(&parts.headers).unwrap_or_else(|_| Uuid::new_v4());
    let result = async {
        policy::correlation(&parts.headers)?;
        policy::version(&parts.headers)?;
        let selection = Query::<Selection>::try_from_uri(&parts.uri).map_err(|_| {
            crate::Rejection::new(
                ContentErrorCode::InvalidRequest,
                "Invalid operation catalog query",
            )
        })?;
        boundary.operation_catalog(parts, selection.vault_id).await
    }
    .await;
    response::decorate(
        match result {
            Ok(value) => Json(value).into_response(),
            Err(error) => error.response(id),
        },
        id,
        false,
    )
}

async fn invoke(
    State(boundary): State<Boundary>,
    Path((vault, plugin, operation)): Path<(Uuid, String, String)>,
    request: Request,
) -> Response {
    let (parts, body) = request.into_parts();
    let correlation = policy::correlation(&parts.headers);
    let mut id = correlation
        .as_ref()
        .copied()
        .unwrap_or_else(|_| Uuid::new_v4());
    let result = async {
        boundary.check(parts.clone()).await?;
        correlation?;
        policy::version(&parts.headers)?;
        let bytes = axum::body::to_bytes(body, 64 * 1024).await.map_err(|_| {
            crate::Rejection::new(ContentErrorCode::TooLarge, "Operation input exceeds 64 KiB")
        })?;
        let input: OperationRequest = serde_json::from_slice(&bytes).map_err(|_| {
            crate::Rejection::new(
                ContentErrorCode::InvalidRequest,
                "Invalid operation request",
            )
        })?;
        if parts.headers.contains_key("x-lapis-request-id") && id != input.request_id {
            return Err(crate::Rejection::new(
                ContentErrorCode::InvalidRequest,
                "Request identities disagree",
            ));
        }
        id = input.request_id;
        boundary
            .invoke_operation(parts, vault, plugin, operation, input)
            .await
    }
    .await;
    response::decorate(
        match result {
            Ok(value) => {
                let status = match &value.outcome {
                    OperationOutcome::Ok { .. } => axum::http::StatusCode::OK,
                    OperationOutcome::Error { error } => response::status(error.code),
                };
                (status, Json(value)).into_response()
            }
            Err(error) => error.response(id),
        },
        id,
        false,
    )
}
