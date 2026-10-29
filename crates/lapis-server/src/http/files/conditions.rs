use super::super::errors::Failure;
use axum::http::HeaderMap;
use lapis_app_core::vault::WriteOptions;
use lapis_app_protocol::{ErrorCode, ExpectedVersion};

pub(super) fn write(headers: &HeaderMap) -> Result<Option<ExpectedVersion>, Failure> {
    let exact = headers.get("if-match");
    let missing = headers.get("if-none-match");
    if exact.is_some() && missing.is_some() {
        return Err(invalid());
    }
    if let Some(value) = missing {
        return if value == "*" {
            Ok(Some(ExpectedVersion::Missing {}))
        } else {
            Err(invalid())
        };
    }
    if let Some(value) = exact {
        let version = value
            .to_str()
            .ok()
            .and_then(|value| value.strip_prefix('"'))
            .and_then(|value| value.strip_suffix('"'))
            .ok_or_else(invalid)?;
        if version.len() != 64
            || !version
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(invalid());
        }
        return Ok(Some(ExpectedVersion::Version {
            version: version.into(),
        }));
    }
    Ok(None)
}
pub(super) fn options(headers: &HeaderMap) -> Result<WriteOptions, Failure> {
    let mtime = headers
        .get("x-lapis-mtime")
        .map(|value| {
            value
                .to_str()
                .ok()
                .and_then(|value| value.parse::<f64>().ok())
                .ok_or_else(invalid)
        })
        .transpose()?;
    if mtime
        .is_some_and(|value| !value.is_finite() || !(0.0..=253_402_300_799_999.0).contains(&value))
    {
        return Err(invalid());
    }
    Ok(WriteOptions { mtime })
}
pub(in crate::http) fn unmodified(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get("if-none-match")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value.split(',').any(|tag| {
                let tag = tag.trim();
                tag == "*" || tag.strip_prefix("W/").unwrap_or(tag) == etag
            })
        })
}
fn invalid() -> Failure {
    Failure::new(
        ErrorCode::InvalidRequest,
        "Invalid file precondition or timestamp",
    )
}
