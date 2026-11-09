use super::*;
use crate::host::error;
use axum::{
    Json,
    extract::{DefaultBodyLimit, Path, rejection::JsonRejection},
    routing::{delete, get, post},
};
use lapis_app_protocol::{
    ErrorCode,
    content::{ContentAccessSetting, ContentGrantRequest, CreatedContentGrant},
};
use std::future::Future;
use uuid::Uuid;

pub(in crate::http) fn routes() -> Router<State> {
    Router::new()
        .route("/api/v3/content-access", get(read).post(select))
        .route("/api/v3/content-access/grants", post(issue))
        .route("/api/v3/content-access/grants/{grant}", delete(revoke))
        .layer(DefaultBodyLimit::max(8 * 1024))
}

async fn controlled<T, F, Fut>(state: State, action: F) -> Result<T, Failure>
where
    T: Send + 'static,
    F: FnOnce(State) -> Fut + Send + 'static,
    Fut: Future<Output = Result<T, Failure>> + Send + 'static,
{
    let accepted = state.host.content.accept()?;
    tokio::spawn(async move {
        let _accepted = accepted;
        let access = state.host.content.clone();
        let _guard = access.gate.lock().await;
        if !state.host.ready() {
            return Err(error(ErrorCode::Closed, "Host is closing").into());
        }
        action(state).await
    })
    .await
    .map_err(|_| {
        Failure::new(
            ErrorCode::Io,
            "Content control outcome is unavailable; refresh status",
        )
    })?
}
async fn read(AxumState(state): AxumState<State>) -> Result<Json<ContentHostStatus>, Failure> {
    controlled(
        state,
        move |state| async move { Ok(Json(status(state).await?)) },
    )
    .await
}
async fn select(
    AxumState(state): AxumState<State>,
    input: Result<Json<ContentAccessSetting>, JsonRejection>,
) -> Result<Json<ContentHostStatus>, Failure> {
    let input = super::super::routes::body(input)?;
    controlled(state, move |state| async move {
        if input.enabled {
            if boundary(&state.host, &state.origin).is_err() {
                return Err(Failure::new(
                    ErrorCode::Unsupported,
                    "Content access requires an HTTPS public origin or HTTP loopback",
                ));
            }
            if let Some(control) = &state.auth.control {
                control.set_content_enabled(true).await.map_err(store)?;
            }
        }
        let host = state.host.clone();
        let selected = blocking(move || {
            host.content
                .grants
                .set_enabled(input.enabled)
                .map_err(native)
        })
        .await;
        if !input.enabled || selected.is_err() {
            // Fail closed even if persistence fails; a restart never restores tokens.
            if let Some(control) = &state.auth.control {
                control.set_content_enabled(false).await.map_err(store)?;
            }
        }
        selected?;
        Ok(Json(status(state).await?))
    })
    .await
}
async fn issue(
    AxumState(state): AxumState<State>,
    input: Result<Json<ContentGrantRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<CreatedContentGrant>), Failure> {
    let input = super::super::routes::body(input)?;
    controlled(state, move |state| async move {
        let issued = blocking(move || state.host.issue_content(input)).await?;
        Ok((
            StatusCode::CREATED,
            Json(CreatedContentGrant {
                grant_id: issued.grant_id,
                token: issued.token.expose_secret().into(),
            }),
        ))
    })
    .await
}
async fn revoke(
    AxumState(state): AxumState<State>,
    Path(grant): Path<Uuid>,
) -> Result<StatusCode, Failure> {
    controlled(state, move |state| async move {
        blocking(move || state.host.content.grants.revoke(grant).map_err(native)).await?;
        Ok(StatusCode::NO_CONTENT)
    })
    .await
}
fn store(_: io::Error) -> Failure {
    Failure::new(
        ErrorCode::Io,
        "Content preference could not be saved; refresh status before retrying",
    )
}

#[cfg(test)]
mod tests;
