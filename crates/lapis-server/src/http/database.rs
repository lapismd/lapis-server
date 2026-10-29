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
use lapis_app_protocol::database::call::{DatabaseCall, DatabaseValue};
use uuid::Uuid;

pub(super) const JSON_LIMIT: usize = 32 * 1024 * 1024;

pub(super) async fn call(
    AxumState(state): AxumState<State>,
    Path(client): Path<Uuid>,
    Extension(principal): Extension<Principal>,
    input: Result<Json<DatabaseCall>, JsonRejection>,
) -> Result<Json<DatabaseValue>, Failure> {
    let call = body(input)?;
    // Native profile/root checks and bounded argument decoding do not run on
    // the HTTP executor. The shared worker owns accepted persistence.
    blocking(move || {
        tokio::runtime::Handle::current().block_on(state.host.database(client, principal, call))
    })
    .await
    .map(Json)
}
