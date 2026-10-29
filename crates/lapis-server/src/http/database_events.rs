use super::{State, auth, errors::Failure};
use crate::host::Principal;
use axum::{
    Extension,
    extract::{Path, Query, State as AxumState},
    http::HeaderMap,
    response::{IntoResponse, Response, Sse, sse::Event},
};
use futures_util::stream;
use lapis_app_protocol::{
    ErrorCode,
    database::{
        call::{DatabaseCall, DatabaseMethod, DatabaseValue},
        changes::ChangeSet,
    },
};
use serde::Deserialize;
use std::{convert::Infallible, time::Duration};
use tokio::sync::{OwnedSemaphorePermit, broadcast};
use uuid::Uuid;

const MAX_FRAME: usize = 256 * 1024;

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
    changes: broadcast::Receiver<ChangeSet>,
    pending: Option<ChangeSet>,
    revision: u64,
    _permit: OwnedSemaphorePermit,
}

impl Observer {
    async fn current(&self) -> Result<u64, Failure> {
        auth::validate_stream(&self.state, self.principal, &self.headers).await?;
        let client = self.state.host.observation_client(
            self.client,
            self.principal,
            self.scope.vault_id,
            self.scope.generation,
        )?;
        let value = client
            .call_database(DatabaseCall {
                method: DatabaseMethod::GetChangeRevision,
                args: vec![],
            })
            .await
            .map_err(|error| Failure::from(error.0))?;
        match value {
            DatabaseValue::Value { value } => value.as_u64().ok_or_else(|| {
                Failure::new(
                    ErrorCode::Io,
                    "Database observation revision is unavailable",
                )
            }),
            _ => Err(Failure::new(
                ErrorCode::Io,
                "Database revision is unavailable",
            )),
        }
    }

    async fn next(&mut self) -> Option<Event> {
        loop {
            let change = if let Some(reset) = self.pending.take() {
                reset
            } else {
                tokio::select! {
                    value = self.changes.recv() => match value {
                        Ok(change) => change,
                        Err(broadcast::error::RecvError::Lagged(_)) => reset(self.current().await.ok()?),
                        Err(broadcast::error::RecvError::Closed) => return None,
                    },
                    () = tokio::time::sleep(Duration::from_secs(1)) => {
                        self.current().await.ok()?;
                        return Some(Event::default().comment("keep-alive"));
                    },
                }
            };
            // Even an already-buffered commit cannot outlive its admitted caller.
            self.current().await.ok()?;
            if change.revision < self.revision
                || (change.revision == self.revision && change.reset != Some(true))
            {
                continue;
            }
            let mut data = serde_json::to_string(&change).ok()?;
            if data.len() > MAX_FRAME
                || change.paths.len() > 4096
                || change
                    .renamed
                    .as_ref()
                    .is_some_and(|paths| paths.len() > 4096)
            {
                data = serde_json::to_string(&reset(self.current().await.ok()?)).ok()?;
            }
            let revision = serde_json::from_str::<ChangeSet>(&data).ok()?.revision;
            self.revision = revision;
            return Some(
                Event::default()
                    .event("change")
                    .id(revision.to_string())
                    .data(data),
            );
        }
    }
}

fn reset(revision: u64) -> ChangeSet {
    ChangeSet {
        revision,
        domains: vec![],
        paths: vec![],
        renamed: None,
        reset: Some(true),
        committed_at: crate::control::now_ms().unwrap_or(0),
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
        .map_err(|_| Failure::new(ErrorCode::Busy, "Database observation reader limit reached"))?;
    let changes = state
        .host
        .observation_client(client, principal, scope.vault_id, scope.generation)?
        .subscribe_to_database_changes()
        .await
        .map_err(|error| Failure::from(error.0))?;
    let mut observer = Observer {
        state,
        principal,
        headers,
        client,
        scope,
        changes,
        pending: None,
        revision: 0,
        _permit: permit,
    };
    let current = observer.current().await?;
    if observer.scope.cursor.is_some_and(|cursor| cursor > current) {
        return Err(Failure::new(
            ErrorCode::InvalidRequest,
            "Observation cursor is ahead",
        ));
    }
    observer.revision = current;
    observer.pending = Some(reset(current));
    let frames = stream::unfold(observer, |mut observer| async move {
        let event = observer.next().await?;
        Some((Ok::<_, Infallible>(event), observer))
    });
    let mut response = Sse::new(frames).into_response();
    response
        .headers_mut()
        .insert("cache-control", "no-store".parse().unwrap());
    Ok(response)
}
