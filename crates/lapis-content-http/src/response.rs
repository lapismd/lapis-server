use crate::{API_HEADER, REQUEST_ID_HEADER};
use axum::{
    Json,
    http::{HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use lapis_app_protocol::content::*;
use uuid::Uuid;

#[derive(Debug)]
pub struct Failure {
    status: StatusCode,
    error: ContentError,
}
fn status(code: ContentErrorCode) -> StatusCode {
    match code {
        ContentErrorCode::InvalidRequest => StatusCode::BAD_REQUEST,
        ContentErrorCode::Unauthenticated => StatusCode::UNAUTHORIZED,
        ContentErrorCode::PermissionDenied => StatusCode::FORBIDDEN,
        ContentErrorCode::NotFound => StatusCode::NOT_FOUND,
        ContentErrorCode::ProtocolMismatch
        | ContentErrorCode::Conflict
        | ContentErrorCode::AlreadyExists => StatusCode::CONFLICT,
        ContentErrorCode::TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
        ContentErrorCode::Busy | ContentErrorCode::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        ContentErrorCode::Unsupported => StatusCode::NOT_IMPLEMENTED,
        ContentErrorCode::OutcomeUnknown => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
impl From<ContentError> for Failure {
    fn from(error: ContentError) -> Self {
        Self {
            status: status(error.code),
            error,
        }
    }
}
impl Failure {
    pub fn new(code: ContentErrorCode, message: &str) -> Self {
        ContentError {
            code,
            message: message.into(),
            effect: ContentEffectStatus::NotStarted,
        }
        .into()
    }
    pub(crate) fn invalid(message: &str) -> Self {
        Self::new(ContentErrorCode::InvalidRequest, message)
    }
    pub(crate) fn method() -> Self {
        let mut failure = Self::new(ContentErrorCode::Unsupported, "Only GET is supported");
        failure.status = StatusCode::METHOD_NOT_ALLOWED;
        failure
    }
    pub fn into_error(self) -> ContentError {
        self.error
    }
    pub fn response(self, id: Uuid) -> Response {
        let mut response = (
            self.status,
            Json(ContentReply {
                api_version: CONTENT_API_MAJOR,
                request_id: id,
                outcome: ContentOutcome::Error { error: self.error },
            }),
        )
            .into_response();
        if self.status == StatusCode::UNAUTHORIZED {
            response.headers_mut().insert(
                "www-authenticate",
                HeaderValue::from_static("Bearer realm=\"lapis-content\""),
            );
        }
        if self.status == StatusCode::METHOD_NOT_ALLOWED {
            response
                .headers_mut()
                .insert("allow", HeaderValue::from_static("GET"));
        }
        response
    }
}
pub(crate) fn reply(reply: ContentReply) -> Response {
    match reply.outcome {
        ContentOutcome::Error { error } => Failure::from(error).response(reply.request_id),
        ContentOutcome::Ok {
            value: ContentValue::Note { text, version, .. },
        } => {
            let Ok(etag) = HeaderValue::from_str(&format!("\"{version}\"")) else {
                return Failure::new(ContentErrorCode::Unavailable, "Content version unavailable")
                    .response(reply.request_id);
            };
            let mut response = text.into_response();
            response.headers_mut().insert("etag", etag);
            response
        }
        outcome => Json(ContentReply { outcome, ..reply }).into_response(),
    }
}
pub(crate) fn decorate(mut response: Response, id: Uuid, close: bool) -> Response {
    let headers = response.headers_mut();
    headers.insert(API_HEADER, HeaderValue::from_static("1"));
    headers.insert(
        REQUEST_ID_HEADER,
        id.to_string().parse().expect("UUID is a header value"),
    );
    headers.insert("cache-control", HeaderValue::from_static("no-store"));
    headers.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    if close {
        headers.insert("connection", HeaderValue::from_static("close"));
    }
    response
}
