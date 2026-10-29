use crate::control::Control;
use sha2::{Digest, Sha256};
use std::{io, sync::Arc};
use subtle::ConstantTimeEq;

#[derive(Clone)]
pub struct Authentication {
    token_hash: Option<[u8; 32]>,
    pub(super) control: Option<Arc<Control>>,
    pub(super) insecure_cookie: bool,
}
impl Authentication {
    pub fn token(token: &str) -> io::Result<Self> {
        Self::settings(None, Some(token), false)
    }
    pub fn browser(
        control: Arc<Control>,
        token: Option<&str>,
        insecure_cookie: bool,
    ) -> io::Result<Self> {
        Self::settings(Some(control), token, insecure_cookie)
    }
    fn settings(
        control: Option<Arc<Control>>,
        token: Option<&str>,
        insecure_cookie: bool,
    ) -> io::Result<Self> {
        if token.is_some_and(|token| {
            token.len() < 32 || token.len() > 4096 || token.bytes().any(|b| !b.is_ascii_graphic())
        }) {
            return Err(io::Error::other(
                "Owner token requires 32 to 4096 non-whitespace ASCII characters",
            ));
        }
        Ok(Self {
            token_hash: token.map(|token| Sha256::digest(token.as_bytes()).into()),
            control,
            insecure_cookie,
        })
    }
    pub fn control(&self) -> Option<Arc<Control>> {
        self.control.clone()
    }
    pub(super) fn matches(&self, token: &str) -> bool {
        self.token_hash.is_some_and(|expected| {
            let digest: [u8; 32] = Sha256::digest(token.as_bytes()).into();
            bool::from(expected.ct_eq(&digest))
        })
    }
    pub(super) fn validate(&self, origin: &axum::http::Uri) -> io::Result<()> {
        if self.control.is_some()
            && ((self.insecure_cookie
                && (origin.scheme_str() != Some("http") || origin.host() != Some("localhost")))
                || (!self.insecure_cookie && origin.scheme_str() != Some("https")))
        {
            return Err(io::Error::other(
                "Browser authentication requires HTTPS; explicit insecure cookies are limited to http://localhost",
            ));
        }
        Ok(())
    }
}
