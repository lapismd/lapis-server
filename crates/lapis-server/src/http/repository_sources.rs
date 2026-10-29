use super::{State, history_reader, routes::body};
use crate::host::Principal;
use axum::{
    Extension, Json,
    extract::{Path, State as AxumState, rejection::JsonRejection},
    response::{IntoResponse, Response},
};
use lapis_app_protocol::repository_sources::RepositorySourcesCommand;
use uuid::Uuid;

pub(super) async fn call(
    AxumState(state): AxumState<State>,
    Path(client): Path<Uuid>,
    Extension(principal): Extension<Principal>,
    input: Result<Json<RepositorySourcesCommand>, JsonRejection>,
) -> Response {
    let command = match body(input) {
        Ok(command) => command,
        Err(error) => return error.into_response(),
    };
    state
        .host
        .repository_sources(client, principal, command)
        .await
        .map(Json)
        .map_err(history_reader::Failure)
        .into_response()
}
