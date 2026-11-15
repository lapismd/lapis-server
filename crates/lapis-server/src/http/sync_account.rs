use super::{State, auth, browser, errors::Failure};
use crate::control::{AccountConnectionDecision, Session, now_ms};
use axum::{
    Json,
    body::to_bytes,
    extract::{Path, Request, State as AxumState},
};
use lapis_app_protocol::ErrorCode;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Initiate {
    client_id: Uuid,
    client_name: String,
    s256_challenge: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Decision {
    approve: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Redeem {
    verifier: String,
}

async fn input<T: serde::de::DeserializeOwned>(request: Request) -> Result<T, Failure> {
    let bytes = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        to_bytes(request.into_body(), 4096),
    )
    .await
    .map_err(|_| invalid())?
    .map_err(|_| invalid())?;
    serde_json::from_slice(&bytes).map_err(|_| invalid())
}

fn invalid() -> Failure {
    Failure::new(
        ErrorCode::InvalidRequest,
        "Invalid account connection request",
    )
}

pub(super) fn store(error: std::io::Error) -> Failure {
    match error.kind() {
        std::io::ErrorKind::WouldBlock => {
            Failure::new(ErrorCode::Busy, "Account connection capacity reached")
        }
        std::io::ErrorKind::InvalidInput => invalid(),
        std::io::ErrorKind::PermissionDenied => Failure::new(
            ErrorCode::PermissionDenied,
            "Account connection is not authorized",
        ),
        _ => browser::store(error),
    }
}

// Native handoff requests carry no browser cookie or ambient account authority.
fn native(state: &State, request: &Request) -> Result<(), Failure> {
    auth::origin(request.headers(), &state.origin, false)?;
    auth::version(request.headers(), false)?;
    if request.headers().contains_key("cookie") || request.headers().contains_key("authorization") {
        return Err(invalid());
    }
    Ok(())
}

pub(super) async fn initiate(
    AxumState(state): AxumState<State>,
    request: Request,
) -> Result<Json<Value>, Failure> {
    native(&state, &request)?;
    let control = state
        .auth
        .control
        .as_ref()
        .ok_or_else(Failure::unauthorized)?;
    let body: Initiate = input(request).await?;
    let connection = control
        .create_account_connection(
            body.client_id,
            &body.client_name,
            &body.s256_challenge,
            now_ms().map_err(store)?,
        )
        .await
        .map_err(store)?;
    Ok(Json(
        json!({"id":connection.id,"client_id":connection.client_id,
        "client_name":connection.client_name,"expires_at":connection.expires_at}),
    ))
}

pub(super) async fn decide(
    AxumState(state): AxumState<State>,
    Path(id): Path<Uuid>,
    request: Request,
) -> Result<Json<Value>, Failure> {
    // auth::authorize has already checked origin, CSRF and the cookie. An
    // installation token is deliberately insufficient for owner consent.
    let session = request
        .extensions()
        .get::<Session>()
        .ok_or_else(Failure::unauthorized)?
        .id;
    let control = state
        .auth
        .control
        .as_ref()
        .ok_or_else(Failure::unauthorized)?;
    let body: Decision = input(request).await?;
    control
        .decide_account_connection(
            id,
            session,
            if body.approve {
                AccountConnectionDecision::Approve
            } else {
                AccountConnectionDecision::Reject
            },
            now_ms().map_err(store)?,
        )
        .await
        .map_err(store)?;
    Ok(Json(json!({"approved":body.approve})))
}

pub(super) async fn inspect(
    AxumState(state): AxumState<State>,
    Path(id): Path<Uuid>,
    request: Request,
) -> Result<Json<Value>, Failure> {
    request
        .extensions()
        .get::<Session>()
        .ok_or_else(Failure::unauthorized)?;
    let connection = state
        .auth
        .control
        .as_ref()
        .ok_or_else(Failure::unauthorized)?
        .inspect_account_connection(id, now_ms().map_err(store)?)
        .await
        .map_err(store)?
        .ok_or_else(|| Failure::new(ErrorCode::NotFound, "Account connection was not found"))?;
    let status = match connection.status {
        crate::control::AccountConnectionStatus::Pending => "pending",
        crate::control::AccountConnectionStatus::Approved => "approved",
        crate::control::AccountConnectionStatus::Rejected => "rejected",
        crate::control::AccountConnectionStatus::Consumed => "consumed",
        crate::control::AccountConnectionStatus::Expired => "expired",
    };
    Ok(Json(
        json!({"id":connection.id,"client_id":connection.client_id,
        "client_name":connection.client_name,"expires_at":connection.expires_at,"status":status}),
    ))
}

pub(super) async fn redeem(
    AxumState(state): AxumState<State>,
    Path(id): Path<Uuid>,
    request: Request,
) -> Result<Json<Value>, Failure> {
    native(&state, &request)?;
    let control = state
        .auth
        .control
        .as_ref()
        .ok_or_else(Failure::unauthorized)?;
    let body: Redeem = input(request).await?;
    let issued = control
        .redeem_account_connection(id, &body.verifier, now_ms().map_err(store)?)
        .await
        .map_err(store)?;
    Ok(Json(
        json!({"id":issued.credential.id,"client_id":issued.credential.client_id,
        "expires_at":issued.credential.expires_at,"token":issued.token,"scope":"discovery"}),
    ))
}
