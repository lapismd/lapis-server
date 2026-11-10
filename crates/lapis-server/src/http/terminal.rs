use super::{
    State, auth,
    errors::{Failure, blocking},
    routes::body,
};
use crate::host::Principal;
use axum::{
    Extension, Json,
    extract::{Path, Query, State as AxumState, rejection::JsonRejection},
    http::HeaderMap,
    response::{IntoResponse, Response, Sse, sse::Event},
};
use futures_util::stream;
use lapis_app_protocol::{
    ErrorCode,
    terminal::{TerminalCommand, TerminalValue},
};
use serde::Deserialize;
use std::{
    convert::Infallible,
    time::{Duration, Instant},
};
use tokio::sync::OwnedSemaphorePermit;
use uuid::Uuid;

pub(super) async fn call(
    AxumState(state): AxumState<State>,
    Path(client): Path<Uuid>,
    Extension(principal): Extension<Principal>,
    input: Result<Json<TerminalCommand>, JsonRejection>,
) -> Result<Json<TerminalValue>, Failure> {
    let command = body(input)?;
    blocking(move || state.host.terminal(client, principal, command))
        .await
        .map(Json)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Cursor {
    cursor: Option<u64>,
}
struct Observer {
    state: State,
    principal: Principal,
    headers: HeaderMap,
    client: Uuid,
    session: Uuid,
    cursor: u64,
    done: bool,
    deadline: Instant,
    remaining: u8,
    _permit: OwnedSemaphorePermit,
}
impl Observer {
    async fn admitted(&self) -> Option<()> {
        if auth::validate_stream(&self.state, self.principal, &self.headers)
            .await
            .is_err()
        {
            let host = self.state.host.clone();
            let principal = self.principal;
            let _ = blocking(move || host.detach_principal(principal)).await;
            return None;
        }
        Some(())
    }
    async fn next(&mut self) -> Option<Event> {
        if self.done || self.remaining == 0 || Instant::now() >= self.deadline {
            return None;
        }
        self.admitted().await?;
        let host = self.state.host.clone();
        let (client, principal, session, cursor) =
            (self.client, self.principal, self.session, self.cursor);
        let output = blocking(move || host.terminal_output(client, principal, session, cursor))
            .await
            .ok()?;
        // Revalidate after the bounded blocking read, before releasing bytes.
        self.admitted().await?;
        self.state.host.heartbeat(client, principal).ok()?;
        self.cursor = output.cursor;
        // A finite response releases scarce browser HTTP/1 connections even
        // while the shell remains alive. Resume uses the replay byte cursor.
        self.remaining -= 1;
        self.done = output.bytes.is_empty();
        Some(
            Event::default()
                .event("output")
                .data(serde_json::to_string(&output).ok()?),
        )
    }
}
pub(super) async fn observe(
    AxumState(state): AxumState<State>,
    Path((client, session)): Path<(Uuid, Uuid)>,
    Extension(principal): Extension<Principal>,
    Query(cursor): Query<Cursor>,
    headers: HeaderMap,
) -> Result<Response, Failure> {
    let permit = state
        .observations
        .clone()
        .try_acquire_owned()
        .map_err(|_| Failure::new(ErrorCode::Busy, "Terminal observer limit reached"))?;
    // Reject foreign/retired clients before opening a successful stream.
    state.host.heartbeat(client, principal)?;
    let mut observer = Observer {
        state,
        principal,
        headers,
        client,
        session,
        cursor: cursor.cursor.unwrap_or(0),
        done: false,
        deadline: Instant::now() + Duration::from_secs(1),
        remaining: 16,
        _permit: permit,
    };
    let first = observer
        .next()
        .await
        .ok_or_else(|| Failure::new(ErrorCode::NotFound, "Terminal output is unavailable"))?;
    let frames = stream::unfold(
        (observer, Some(first)),
        |(mut observer, first)| async move {
            let event = match first {
                Some(event) => event,
                None => observer.next().await?,
            };
            Some((Ok::<_, Infallible>(event), (observer, None)))
        },
    );
    Ok(Sse::new(frames).into_response())
}
