use super::{State, errors::Failure, routes::body};
use crate::host::Principal;
use axum::{
    Extension, Json,
    extract::{Path, State as AxumState, rejection::JsonRejection},
};
use lapis_app_protocol::native_tasks::{NativeTaskCommand, NativeTaskValue};
use uuid::Uuid;

pub(super) async fn call(
    AxumState(state): AxumState<State>,
    Path(client): Path<Uuid>,
    Extension(principal): Extension<Principal>,
    input: Result<Json<NativeTaskCommand>, JsonRejection>,
) -> Result<Json<NativeTaskValue>, Failure> {
    state
        .host
        .tasks(client, principal, body(input)?)
        .await
        .map(Json)
        .map_err(Into::into)
}
