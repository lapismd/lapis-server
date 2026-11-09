use crate::response::Failure;
use axum::{extract::Query, http::Uri};
use lapis_app_protocol::content::*;
use serde::{Deserialize, de::DeserializeOwned};
use uuid::Uuid;

/// Paths are relative to `/api/content/v1`; checked against live Core discovery.
pub const READ_ROUTES: &[(ContentOperation, &str)] = &[
    (ContentOperation::Capabilities, "/capabilities"),
    (ContentOperation::ListVaults, "/vaults"),
    (ContentOperation::ListNotes, "/vaults/{vault_id}/notes"),
    (ContentOperation::ReadNote, "/vaults/{vault_id}/note"),
];
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Page {
    limit: Option<u16>,
    cursor: Option<String>,
}
impl Page {
    fn query(self) -> ContentPageQuery {
        ContentPageQuery {
            limit: self.limit.unwrap_or(50),
            cursor: self.cursor,
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Notes {
    directory: Option<String>,
    limit: Option<u16>,
    cursor: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Note {
    path: String,
}

pub(crate) fn command(uri: &Uri) -> Result<ContentCommand, Failure> {
    match uri.path() {
        "/capabilities" => {
            query::<Empty>(uri)?;
            return Ok(ContentCommand::Capabilities {});
        }
        "/vaults" => {
            return Ok(ContentCommand::ListVaults {
                page: query::<Page>(uri)?.query(),
            });
        }
        _ => {}
    }
    let (vault, route) = uri
        .path()
        .strip_prefix("/vaults/")
        .and_then(|s| s.split_once('/'))
        .ok_or_else(|| Failure::new(ContentErrorCode::NotFound, "Content route not found"))?;
    let vault_id =
        Uuid::parse_str(vault).map_err(|_| Failure::invalid("Invalid vault identity"))?;
    match route {
        "notes" => {
            let Notes {
                directory,
                limit,
                cursor,
            } = query(uri)?;
            Ok(ContentCommand::ListNotes {
                vault_id,
                directory,
                page: Page { limit, cursor }.query(),
            })
        }
        "note" => Ok(ContentCommand::ReadNote {
            vault_id,
            path: query::<Note>(uri)?.path,
        }),
        _ => Err(Failure::new(
            ContentErrorCode::NotFound,
            "Content route not found",
        )),
    }
}
fn query<T: DeserializeOwned>(uri: &Uri) -> Result<T, Failure> {
    let invalid = || Failure::invalid("Invalid content query");
    // serde's URL form decoder replaces invalid UTF-8; reject it before decoding.
    let mut bytes = Vec::new();
    let mut encoded = uri.query().unwrap_or("").bytes();
    while let Some(byte) = encoded.next() {
        bytes.push(if byte == b'%' {
            let high = encoded
                .next()
                .and_then(|b| (b as char).to_digit(16))
                .ok_or_else(invalid)?;
            let low = encoded
                .next()
                .and_then(|b| (b as char).to_digit(16))
                .ok_or_else(invalid)?;
            (high * 16 + low) as u8
        } else {
            byte
        });
    }
    std::str::from_utf8(&bytes).map_err(|_| invalid())?;
    Query::<T>::try_from_uri(uri)
        .map(|query| query.0)
        .map_err(|_| invalid())
}
