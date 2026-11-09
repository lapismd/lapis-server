use super::{Result, failure};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use lapis_app_protocol::content::ContentErrorCode;
use sha2::{Digest, Sha256};
use std::fmt;
use uuid::Uuid;

/// Returned only at native issuance. Intentionally neither Clone nor Serialize.
#[derive(Debug)]
pub struct IssuedContentGrant {
    pub grant_id: Uuid,
    pub token: ContentToken,
}

/// Explicit secret access prevents accidental Debug/Display/serde disclosure.
pub struct ContentToken(String);

impl fmt::Debug for ContentToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ContentToken([REDACTED])")
    }
}

impl ContentToken {
    pub(super) fn generate() -> Result<Self> {
        let mut bytes = [0; 32];
        getrandom::fill(&mut bytes)
            .map_err(|_| failure(ContentErrorCode::Unavailable, "Content entropy unavailable"))?;
        Ok(Self(format!("lc1_{}", URL_SAFE_NO_PAD.encode(bytes))))
    }

    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

pub(super) fn verifier(bearer: &str) -> Option<[u8; 32]> {
    if bearer.len() != 47
        || !bearer.starts_with("lc1_")
        || !bearer.as_bytes()[4..]
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-')
    {
        return None;
    }
    Some(Sha256::digest(bearer.as_bytes()).into())
}
