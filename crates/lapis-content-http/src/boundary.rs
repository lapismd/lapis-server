//! Shared HTTP admission for content routes and MCP. Clones share native capacity.
use crate::{MAX_NATIVE_JOBS, Rejection, admission, policy};
use axum::http::request::Parts;
use lapis_app_core::services::content::ContentGrants;
use lapis_app_protocol::content::{ContentReply, ContentRequest};
use std::sync::Arc;
use tokio::sync::Semaphore;

#[derive(Clone)]
pub struct Boundary {
    grants: Arc<ContentGrants>,
    policy: Arc<policy::Policy>,
    jobs: Arc<Semaphore>,
    origin: Arc<str>,
}
impl Boundary {
    pub fn new(grants: Arc<ContentGrants>, origin: &str) -> std::io::Result<Self> {
        Ok(Self {
            grants,
            policy: Arc::new(policy::Policy::new(origin)?),
            jobs: Arc::new(Semaphore::new(MAX_NATIVE_JOBS)),
            origin: origin.into(),
        })
    }
    pub fn origin(&self) -> &str {
        &self.origin
    }
    /// Authenticate before parsing a protocol body. This returns no authority;
    /// execution checks the credential again after native worker admission.
    pub async fn check(&self, parts: Parts) -> Result<Parts, Rejection> {
        self.run(parts, |parts, _, _| Ok(parts)).await
    }
    pub async fn execute(
        &self,
        parts: Parts,
        request: ContentRequest,
    ) -> Result<ContentReply, Rejection> {
        self.run(parts, |_, grants, bearer| {
            Ok(grants.execute(bearer, request))
        })
        .await
    }
    pub(crate) async fn run<T: Send + 'static>(
        &self,
        parts: Parts,
        action: impl FnOnce(Parts, &ContentGrants, &str) -> Result<T, Rejection> + Send + 'static,
    ) -> Result<T, Rejection> {
        let boundary = self.clone();
        admission::run(self.jobs.clone(), move || {
            boundary.policy.check(&parts)?;
            let bearer = policy::bearer(&parts.headers)?.to_owned();
            boundary.grants.check_credential(&bearer)?;
            action(parts, &boundary.grants, &bearer)
        })
        .await
    }
}
