use crate::{
    args::Cli,
    credential,
    errors::{failure, message},
    validation,
};
use lapis_app_protocol::content::*;
use reqwest::{Client, Url, header::HeaderMap};
use std::time::Duration;

const API: &str = "x-lapis-content-api";
const CORRELATION: &str = "x-lapis-request-id";

pub(crate) async fn execute(
    args: &Cli,
    request: &ContentRequest,
) -> Result<ContentReply, ContentError> {
    request.validate()?;
    let mut url = target(args.target.as_deref())?;
    let token = credential::read(args)?;
    route(&mut url, &request.command)?;
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .timeout(Duration::from_millis(args.timeout_ms))
        .build()
        .map_err(|_| failure(ContentErrorCode::Unavailable))?;
    let mut response = client
        .get(url)
        .bearer_auth(token)
        .header(API, "1")
        .header(CORRELATION, request.request_id.to_string())
        .send()
        .await
        .map_err(|_| failure(ContentErrorCode::Unavailable))?;
    if response.status().is_redirection() {
        return Err(failure(ContentErrorCode::ProtocolMismatch));
    }
    if header(response.headers(), API)? != "1"
        || header(response.headers(), CORRELATION)? != request.request_id.to_string()
    {
        return Err(failure(ContentErrorCode::ProtocolMismatch));
    }
    let success = response.status() == reqwest::StatusCode::OK;
    let note = success && matches!(request.command, ContentCommand::ReadNote { .. });
    let limit = if note {
        MAX_CONTENT_TEXT_BYTES
    } else {
        2 * 1024 * 1024
    };
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(failure(ContentErrorCode::TooLarge));
    }
    let content_type = header(response.headers(), "content-type")?
        .split(';')
        .next()
        .unwrap_or("")
        .trim();
    if content_type
        != if note {
            "text/plain"
        } else {
            "application/json"
        }
    {
        return Err(failure(ContentErrorCode::ProtocolMismatch));
    }
    let version = if note {
        Some(validation::version(header(response.headers(), "etag")?)?)
    } else {
        None
    };
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| failure(ContentErrorCode::Unavailable))?
    {
        if chunk.len() > limit - bytes.len() {
            return Err(failure(ContentErrorCode::TooLarge));
        }
        bytes.extend_from_slice(&chunk);
    }
    if let ContentCommand::ReadNote { path, .. } = &request.command
        && let Some(version) = version
    {
        let text =
            String::from_utf8(bytes).map_err(|_| failure(ContentErrorCode::ProtocolMismatch))?;
        return Ok(ContentReply {
            api_version: CONTENT_API_MAJOR,
            request_id: request.request_id,
            outcome: ContentOutcome::Ok {
                value: ContentValue::Note {
                    path: path.clone(),
                    text,
                    version,
                },
            },
        });
    }
    let mut reply: ContentReply =
        serde_json::from_slice(&bytes).map_err(|_| failure(ContentErrorCode::ProtocolMismatch))?;
    if reply.api_version != CONTENT_API_MAJOR || reply.request_id != request.request_id {
        return Err(failure(ContentErrorCode::ProtocolMismatch));
    }
    match &mut reply.outcome {
        ContentOutcome::Ok { value } if success => validation::value(value, &request.command)?,
        ContentOutcome::Error { error } if !success => error.message = message(error.code).into(),
        _ => return Err(failure(ContentErrorCode::ProtocolMismatch)),
    }
    Ok(reply)
}

pub(crate) fn target(input: Option<&str>) -> Result<Url, ContentError> {
    let invalid = || failure(ContentErrorCode::InvalidRequest);
    let url = Url::parse(input.ok_or_else(invalid)?).map_err(|_| invalid())?;
    let loopback = url.host_str().is_some_and(|host| {
        host == "localhost"
            || host
                .trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    });
    if !(url.scheme() == "https" || url.scheme() == "http" && loopback)
        || url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid());
    }
    Ok(url)
}
fn header<'a>(headers: &'a HeaderMap, key: &str) -> Result<&'a str, ContentError> {
    let invalid = || failure(ContentErrorCode::ProtocolMismatch);
    let mut values = headers.get_all(key).iter();
    let value = values
        .next()
        .ok_or_else(invalid)?
        .to_str()
        .map_err(|_| invalid())?;
    if values.next().is_some() {
        return Err(invalid());
    }
    Ok(value)
}
fn route(url: &mut Url, command: &ContentCommand) -> Result<(), ContentError> {
    let (path, page) = match command {
        ContentCommand::Capabilities {} => ("/api/content/v1/capabilities".into(), None),
        ContentCommand::ListVaults { page } => ("/api/content/v1/vaults".into(), Some(page)),
        ContentCommand::ListNotes { vault_id, page, .. } => (
            format!("/api/content/v1/vaults/{vault_id}/notes"),
            Some(page),
        ),
        ContentCommand::ReadNote { vault_id, .. } => {
            (format!("/api/content/v1/vaults/{vault_id}/note"), None)
        }
        _ => return Err(failure(ContentErrorCode::Unsupported)),
    };
    url.set_path(&path);
    if let Some(page) = page {
        url.query_pairs_mut()
            .append_pair("limit", &page.limit.to_string());
        if let Some(cursor) = &page.cursor {
            url.query_pairs_mut().append_pair("cursor", cursor);
        }
    }
    match command {
        ContentCommand::ReadNote { path, .. } => {
            url.query_pairs_mut().append_pair("path", path);
        }
        ContentCommand::ListNotes {
            directory: Some(directory),
            ..
        } => {
            url.query_pairs_mut().append_pair("directory", directory);
        }
        _ => {}
    }
    Ok(())
}
