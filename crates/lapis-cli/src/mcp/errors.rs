use crate::errors::{failure, message};
use lapis_app_protocol::content::*;
use rmcp::{
    ErrorData, ServiceError,
    service::ClientInitializeError,
    transport::{DynamicTransportError, streamable_http_client::StreamableHttpError},
};

pub fn startup(error: ClientInitializeError) -> ContentError {
    match error {
        ClientInitializeError::TransportError { error, .. } => failure(transport(&error)),
        ClientInitializeError::ConnectionClosed(_) | ClientInitializeError::Cancelled => {
            failure(ContentErrorCode::Unavailable)
        }
        _ => failure(ContentErrorCode::ProtocolMismatch),
    }
}
pub fn request(error: ServiceError) -> ErrorData {
    match error {
        ServiceError::McpError(error) => {
            let data = error
                .data
                .and_then(|v| serde_json::from_value::<ContentError>(v).ok())
                .map(|mut e| {
                    e.message = message(e.code).into();
                    serde_json::to_value(e).unwrap()
                });
            ErrorData::new(error.code, "Content MCP request failed", data)
        }
        ServiceError::TransportSend(error) => local(transport(&error)),
        ServiceError::UnexpectedResponse => local(ContentErrorCode::ProtocolMismatch),
        _ => local(ContentErrorCode::Unavailable),
    }
}
pub fn local(code: ContentErrorCode) -> ErrorData {
    ErrorData::new(
        rmcp::model::ErrorCode(-32000),
        message(code),
        Some(serde_json::json!(failure(code))),
    )
}
fn transport(error: &DynamicTransportError) -> ContentErrorCode {
    match error
        .error
        .downcast_ref::<StreamableHttpError<reqwest_mcp::Error>>()
    {
        Some(StreamableHttpError::AuthRequired(_)) => ContentErrorCode::Unauthenticated,
        Some(StreamableHttpError::InsufficientScope(_)) => ContentErrorCode::PermissionDenied,
        Some(StreamableHttpError::ResponseBodyTooLarge { .. }) => ContentErrorCode::TooLarge,
        Some(
            StreamableHttpError::Deserialize(_)
            | StreamableHttpError::UnexpectedContentType(_)
            | StreamableHttpError::UnexpectedServerResponse(_),
        ) => ContentErrorCode::ProtocolMismatch,
        _ => ContentErrorCode::Unavailable,
    }
}
