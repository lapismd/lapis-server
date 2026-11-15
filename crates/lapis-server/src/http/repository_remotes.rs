use super::{State, errors::Failure, routes::body};
use crate::host::Principal;
use axum::{
    Extension, Json,
    extract::{Path, Query, State as AxumState, rejection::JsonRejection},
};
use lapis_app_protocol::repository_remotes::{RepositoryRemotesCommand, RepositoryRemotesSnapshot};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Scope {
    vault: Uuid,
    generation: Uuid,
}

pub(super) async fn settings(
    AxumState(state): AxumState<State>,
    Path(client): Path<Uuid>,
    Query(scope): Query<Scope>,
    Extension(principal): Extension<Principal>,
) -> Result<Json<RepositoryRemotesSnapshot>, Failure> {
    state
        .host
        .repository_remotes(client, principal, scope.vault, scope.generation, None)
        .await
        .map(Json)
        .map_err(Into::into)
}

pub(super) async fn control(
    AxumState(state): AxumState<State>,
    Path(client): Path<Uuid>,
    Query(scope): Query<Scope>,
    Extension(principal): Extension<Principal>,
    input: Result<Json<RepositoryRemotesCommand>, JsonRejection>,
) -> Result<Json<RepositoryRemotesSnapshot>, Failure> {
    state
        .host
        .repository_remotes(
            client,
            principal,
            scope.vault,
            scope.generation,
            Some(body(input)?),
        )
        .await
        .map(Json)
        .map_err(Into::into)
}

pub(super) async fn working_copy(
    AxumState(state): AxumState<State>,
    Path(client): Path<Uuid>,
    Query(scope): Query<Scope>,
    Extension(principal): Extension<Principal>,
) -> Result<impl axum::response::IntoResponse, Failure> {
    state
        .host
        .repository_working_copy(client, principal, scope.vault, scope.generation)
        .await
        .map(|text| ([("content-type", "application/json")], text))
        .map_err(Into::into)
}
