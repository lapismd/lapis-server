use crate::{API_HEADER, REQUEST_ID_HEADER, response::Failure};
use axum::http::{HeaderMap, Uri, request::Parts};
use lapis_app_protocol::content::ContentErrorCode;
use std::io;
use uuid::Uuid;

pub(crate) struct Policy {
    origin: String,
    authority: String,
    scheme: String,
}
impl Policy {
    pub(crate) fn new(origin: &str) -> io::Result<Self> {
        let invalid =
            || io::Error::other("Configure an exact HTTPS origin or HTTP loopback origin");
        let uri: Uri = origin.parse().map_err(|_| invalid())?;
        let scheme = uri.scheme_str().ok_or_else(invalid)?;
        let authority = uri.authority().ok_or_else(invalid)?;
        let host = uri.host().ok_or_else(invalid)?;
        let loopback = host == "localhost"
            || host
                .trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback());
        if origin.len() > 2048
            || origin.ends_with('/')
            || origin.contains(['@', '#', '?'])
            || uri.path_and_query().is_some_and(|p| p.as_str() != "/")
            || !(scheme == "https" || scheme == "http" && loopback)
        {
            return Err(invalid());
        }
        Ok(Self {
            origin: origin.into(),
            authority: authority.as_str().into(),
            scheme: scheme.into(),
        })
    }
    pub(crate) fn check(&self, parts: &Parts) -> Result<(), Failure> {
        let uri = parts
            .extensions
            .get::<axum::extract::OriginalUri>()
            .map_or(&parts.uri, |original| &original.0);
        if uri.to_string().len() > 16 * 1024
            || parts
                .headers
                .iter()
                .map(|(k, v)| k.as_str().len() + v.as_bytes().len())
                .sum::<usize>()
                > 8 * 1024
        {
            return Err(Failure::new(
                ContentErrorCode::TooLarge,
                "Content request metadata exceeds its limit",
            ));
        }
        let denied = || {
            Failure::new(
                ContentErrorCode::PermissionDenied,
                "Content request origin is not admitted",
            )
        };
        let host = single(&parts.headers, "host")
            .map_err(|_| denied())?
            .ok_or_else(denied)?;
        if !host.eq_ignore_ascii_case(&self.authority) {
            return Err(denied());
        }
        if uri
            .authority()
            .is_some_and(|a| !a.as_str().eq_ignore_ascii_case(&self.authority))
            || uri.scheme_str().is_some_and(|s| s != self.scheme)
        {
            return Err(denied());
        }
        if single(&parts.headers, "origin")
            .map_err(|_| denied())?
            .is_some_and(|origin| origin != self.origin)
        {
            return Err(denied());
        }
        Ok(())
    }
}
fn single<'a>(headers: &'a HeaderMap, name: &str) -> Result<Option<&'a str>, ()> {
    let mut values = headers.get_all(name).iter();
    let value = values
        .next()
        .map(|v| v.to_str().map_err(|_| ()))
        .transpose()?;
    if values.next().is_some() {
        return Err(());
    }
    Ok(value)
}
pub(crate) fn bearer(headers: &HeaderMap) -> Result<&str, Failure> {
    let denied = || {
        Failure::new(
            ContentErrorCode::Unauthenticated,
            "Invalid content credential",
        )
    };
    let value = single(headers, "authorization")
        .map_err(|_| denied())?
        .ok_or_else(denied)?;
    let (scheme, token) = value.split_once(' ').ok_or_else(denied)?;
    if !scheme.eq_ignore_ascii_case("Bearer") {
        return Err(denied());
    }
    Ok(token)
}
pub(crate) fn version(headers: &HeaderMap) -> Result<(), Failure> {
    if single(headers, API_HEADER) != Ok(Some("1")) {
        return Err(Failure::new(
            ContentErrorCode::ProtocolMismatch,
            "Content API major 1 is required",
        ));
    }
    Ok(())
}
pub(crate) fn correlation(headers: &HeaderMap) -> Result<Uuid, Failure> {
    let invalid = || Failure::invalid("Invalid content request ID");
    match single(headers, REQUEST_ID_HEADER).map_err(|_| invalid())? {
        None => Ok(Uuid::new_v4()),
        Some(value) => Uuid::parse_str(value)
            .ok()
            .filter(|id| !id.is_nil())
            .ok_or_else(invalid),
    }
}
pub(crate) fn has_body(headers: &HeaderMap) -> bool {
    headers.contains_key("transfer-encoding")
        || headers
            .get_all("content-length")
            .iter()
            .any(|value| value.as_bytes() != b"0")
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
