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
use lapis_app_protocol::turns::{TurnCommand, TurnValue};
use uuid::Uuid;

// Authored/context bytes are independently bounded by Core before acceptance.
// Allow their JSON escaping without expanding the ordinary command limit.
pub(super) const JSON_LIMIT: usize = 16 * 1024 * 1024;
pub(super) async fn call(
    AxumState(state): AxumState<State>,
    Path(client): Path<Uuid>,
    Extension(principal): Extension<Principal>,
    input: Result<Json<TurnCommand>, JsonRejection>,
) -> Result<Json<TurnValue>, Failure> {
    let command = body(input)?;
    blocking(move || state.host.turn(client, principal, command))
        .await
        .map(Json)
}
