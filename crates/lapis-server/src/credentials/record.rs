use super::sealed::unavailable;
use serde::{Deserialize, Serialize};
use std::io;
use url::Url;
use uuid::Uuid;
use zeroize::Zeroize;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CredentialSummary {
    pub id: Uuid,
    pub label: String,
    pub endpoint: String,
    pub username: String,
}
/// Transient input only. Never include this in inventory, settings or logs.
pub struct TokenInput {
    pub label: String,
    pub endpoint: String,
    pub username: String,
    pub token: String,
}
impl Drop for TokenInput {
    fn drop(&mut self) {
        self.token.zeroize();
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    pub summary: CredentialSummary,
    pub token: String,
}
impl Drop for Record {
    fn drop(&mut self) {
        self.token.zeroize();
    }
}
pub(super) fn endpoint(value: &str) -> io::Result<String> {
    let url = Url::parse(value).map_err(|_| unavailable())?;
    if value.len() > 4096
        || value.chars().any(char::is_control)
        || url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(unavailable());
    }
    Ok(url.to_string().trim_end_matches('/').to_owned())
}
pub(super) fn token(value: &str) -> io::Result<()> {
    if value.is_empty() || value.len() > 4096 || value.chars().any(char::is_control) {
        return Err(unavailable());
    }
    Ok(())
}
impl Record {
    pub fn new(input: &TokenInput, id: Uuid) -> io::Result<Self> {
        token(&input.token)?;
        if input.label.trim().is_empty()
            || input.label.len() > 128
            || input.label.chars().any(char::is_control)
            || input.username.is_empty()
            || input.username.len() > 128
            || input.username.chars().any(|c| c.is_control() || c == ':')
        {
            return Err(unavailable());
        }
        Ok(Self {
            summary: CredentialSummary {
                id,
                label: input.label.clone(),
                endpoint: endpoint(&input.endpoint)?,
                username: input.username.clone(),
            },
            token: input.token.clone(),
        })
    }
}

pub(super) fn matches_scope(approved: &str, requested: &str) -> io::Result<bool> {
    let requested = endpoint(requested)?;
    if approved == requested {
        return Ok(true);
    }
    let approved = Url::parse(approved).map_err(|_| unavailable())?;
    let requested = Url::parse(&requested).map_err(|_| unavailable())?;
    // An origin scope is an explicit host/account grant; the stored username
    // remains authoritative. A repository-path scope never expands to siblings.
    Ok(approved.path() == "/" && approved.origin() == requested.origin())
}
