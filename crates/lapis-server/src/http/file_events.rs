use super::{
    State, auth,
    errors::{Failure, blocking},
};
use crate::host::Principal;
use axum::{
    Extension,
    extract::{Path, Query, State as AxumState},
    http::HeaderMap,
    response::{IntoResponse, Response, Sse, sse::Event},
};
use futures_util::stream;
use lapis_app_core::services::FileObservation;
use lapis_app_protocol::{ErrorCode, file_changes::FileChangeSet};
use serde::Deserialize;
use std::{
    convert::Infallible,
    time::{Duration, Instant},
};
use tokio::sync::{OwnedSemaphorePermit, broadcast};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Scope {
    vault_id: Uuid,
    generation: Uuid,
    cursor: Option<u64>,
}
struct Observer {
    state: State,
    principal: Principal,
    headers: HeaderMap,
    client: Uuid,
    scope: Scope,
    observation: FileObservation,
    ready: bool,
    revision: u64,
    expires: Instant,
    _permit: OwnedSemaphorePermit,
}
impl Observer {
    async fn validate(&self) -> Result<(), Failure> {
        auth::validate_stream(&self.state, self.principal, &self.headers).await?;
        self.state
            .host
            .observation_client(
                self.client,
                self.principal,
                self.scope.vault_id,
                self.scope.generation,
            )?
            .validate_file_observation()
            .map_err(|e| Failure::from(e.0))
    }
    async fn next(&mut self) -> Option<Event> {
        loop {
            if Instant::now() >= self.expires {
                return None;
            }
            self.validate().await.ok()?;
            let initial = !self.ready;
            let frame = if initial {
                self.ready = true;
                self.observation.ready.clone()
            } else if let Some(frame) = self.observation.pending.pop_front() {
                frame
            } else {
                tokio::select! {
                    result = self.observation.changes.recv() => match result {
                        Ok(frame) => frame,
                        Err(broadcast::error::RecvError::Closed) => return None,
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            self.observation = self.state.host.observation_client(self.client, self.principal, self.scope.vault_id, self.scope.generation).ok()?
                                .subscribe_to_file_changes(None).ok()?;
                            FileChangeSet { reset: true, ..self.observation.ready.clone() }
                        }
                    },
                    () = tokio::time::sleep(Duration::from_secs(1)) => {
                        self.validate().await.ok()?;
                        return Some(Event::default().comment("keep-alive"));
                    }
                }
            };
            self.validate().await.ok()?;
            if !initial && frame.revision <= self.revision && !frame.reset {
                continue;
            }
            self.revision = frame.revision;
            return Some(
                Event::default()
                    .event(if initial { "ready" } else { "change" })
                    .id(frame.revision.to_string())
                    .data(serde_json::to_string(&frame).ok()?),
            );
        }
    }
}
pub(super) async fn observe(
    AxumState(state): AxumState<State>,
    Path(client): Path<Uuid>,
    Extension(principal): Extension<Principal>,
    Query(scope): Query<Scope>,
    headers: HeaderMap,
) -> Result<Response, Failure> {
    if scope.cursor.is_some_and(|v| v > 9_007_199_254_740_991) {
        return Err(Failure::new(
            ErrorCode::InvalidRequest,
            "Invalid file observation cursor",
        ));
    }
    let permit = state
        .observations
        .clone()
        .try_acquire_owned()
        .map_err(|_| Failure::new(ErrorCode::Busy, "Observation reader limit reached"))?;
    let admitted =
        state
            .host
            .observation_client(client, principal, scope.vault_id, scope.generation)?;
    let cursor = scope.cursor;
    let observation =
        blocking(move || admitted.subscribe_to_file_changes(cursor).map_err(|e| e.0)).await?;
    let observer = Observer {
        state,
        principal,
        headers,
        client,
        scope,
        observation,
        ready: false,
        revision: 0,
        expires: Instant::now() + Duration::from_secs(30),
        _permit: permit,
    };
    observer.validate().await?;
    let frames = stream::unfold(observer, |mut observer| async move {
        let frame = observer.next().await?;
        Some((Ok::<_, Infallible>(frame), observer))
    });
    let mut response = Sse::new(frames).into_response();
    response
        .headers_mut()
        .insert("cache-control", "no-store".parse().unwrap());
    response
        .headers_mut()
        .insert("x-accel-buffering", "no".parse().unwrap());
    Ok(response)
}
