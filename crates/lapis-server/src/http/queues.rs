use super::{
    State,
    errors::{Failure, blocking},
    routes::body,
};
use crate::host::Principal;
use axum::{
    Extension, Json,
    extract::{Path, State as AxumState, rejection::JsonRejection},
};
use lapis_app_protocol::queues::{QueueCommand, QueueValue};
use uuid::Uuid;
pub(super) async fn call(
    AxumState(state): AxumState<State>,
    Path(client): Path<Uuid>,
    Extension(principal): Extension<Principal>,
    input: Result<Json<QueueCommand>, JsonRejection>,
) -> Result<Json<QueueValue>, Failure> {
    let command = body(input)?;
    blocking(move || state.host.queue(client, principal, command))
        .await
        .map(Json)
}
