pub(super) mod conditions;
pub(super) mod ranges;
mod reads;
mod writes;

use super::{State, errors::Failure};
use crate::host::Principal;
use axum::Extension;
use axum::{
    extract::{Path, Request, State as AxumState},
    http::{
        Version,
        header::{CONNECTION, HeaderValue},
    },
    response::{IntoResponse, Response},
};
use uuid::Uuid;

pub(super) async fn read(
    AxumState(state): AxumState<State>,
    Path((client, path)): Path<(Uuid, String)>,
    Extension(principal): Extension<Principal>,
    request: Request,
) -> Result<Response, Failure> {
    reads::serve(state, client, path, principal, request, false).await
}
pub(super) async fn resource(
    AxumState(state): AxumState<State>,
    Path((client, path)): Path<(Uuid, String)>,
    Extension(principal): Extension<Principal>,
    request: Request,
) -> Result<Response, Failure> {
    reads::serve(state, client, path, principal, request, true).await
}
pub(super) async fn write(
    AxumState(state): AxumState<State>,
    Path((client, path)): Path<(Uuid, String)>,
    Extension(principal): Extension<Principal>,
    request: Request,
) -> Response {
    let version = request.version();
    match writes::receive(state, client, path, principal, request).await {
        Ok(response) => response,
        Err(failure) => {
            let mut response = failure.into_response();
            // An early failure can leave body bytes in flight. Do not pool it.
            if matches!(version, Version::HTTP_10 | Version::HTTP_11) {
                response
                    .headers_mut()
                    .insert(CONNECTION, HeaderValue::from_static("close"));
            }
            response
        }
    }
}
