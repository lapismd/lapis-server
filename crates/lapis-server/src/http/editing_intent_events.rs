use super::{State, auth, errors::Failure};
use crate::host::Principal;
use axum::{
    Extension,
    extract::{Path, Query, State as AxumState},
    http::HeaderMap,
    response::{IntoResponse, Response, Sse, sse::Event},
};
use futures_util::stream;
use lapis_app_protocol::ErrorCode;
use serde::{Deserialize, Serialize};
use std::{
    convert::Infallible,
    time::{Duration, Instant},
};
use tokio::sync::{OwnedSemaphorePermit, watch};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Scope {
    vault_id: Uuid,
    generation: Uuid,
}

/// HTTP framing only; the API observation port carries no claim or file data.
#[derive(Serialize)]
struct Invalidation {
    revision: u64,
}

struct Observer {
    state: State,
    principal: Principal,
    headers: HeaderMap,
    client: Uuid,
    scope: Scope,
    changes: watch::Receiver<u64>,
    ready: bool,
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
            .validate_editing_intent_observation()
            .map_err(|e| Failure::from(e.0))
    }
    async fn next(&mut self) -> Option<Event> {
        if Instant::now() >= self.expires {
            return None;
        }
        self.validate().await.ok()?;
        let initial = !self.ready;
        if !initial {
            tokio::select! {
                result = self.changes.changed() => { result.ok()?; }
                () = tokio::time::sleep(Duration::from_secs(1)) => {
                    self.validate().await.ok()?;
                    return Some(Event::default().comment("keep-alive"));
                }
            }
        }
        self.validate().await.ok()?;
        self.ready = true;
        let frame = Invalidation {
            revision: *self.changes.borrow_and_update(),
        };
        Some(
            Event::default()
                .event(if initial { "ready" } else { "invalidate" })
                .id(frame.revision.to_string())
                .data(serde_json::to_string(&frame).ok()?),
        )
    }
}
pub(super) async fn observe(
    AxumState(state): AxumState<State>,
    Path(client): Path<Uuid>,
    Extension(principal): Extension<Principal>,
    Query(scope): Query<Scope>,
    headers: HeaderMap,
) -> Result<Response, Failure> {
    let permit = state
        .observations
        .clone()
        .try_acquire_owned()
        .map_err(|_| Failure::new(ErrorCode::Busy, "Observation reader limit reached"))?;
    let admitted =
        state
            .host
            .observation_client(client, principal, scope.vault_id, scope.generation)?;
    let changes = admitted
        .subscribe_to_editing_intents()
        .map_err(|e| Failure::from(e.0))?;
    let observer = Observer {
        state,
        principal,
        headers,
        client,
        scope,
        changes,
        ready: false,
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
