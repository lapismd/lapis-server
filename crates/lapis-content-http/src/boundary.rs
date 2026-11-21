//! Shared HTTP admission for content routes and MCP. Clones share native capacity.
use crate::{MAX_NATIVE_JOBS, Rejection, admission, policy};
use axum::http::request::Parts;
use lapis_app_core::services::content::ContentGrants;
use lapis_app_protocol::content::{ContentReply, ContentRequest};
use std::{future::Future, pin::Pin, sync::Arc};
use tokio::sync::Semaphore;

pub type ResolveFuture<'a> = Pin<Box<dyn Future<Output = Result<(), Rejection>> + Send + 'a>>;

pub trait CredentialResolver: Send + Sync {
    fn resolve<'a>(&'a self, bearer: &'a str, path: &'a str) -> ResolveFuture<'a>;
}

#[derive(Clone)]
pub struct Boundary {
    grants: Arc<ContentGrants>,
    policy: Arc<policy::Policy>,
    jobs: Arc<Semaphore>,
    origin: Arc<str>,
    resolver: Option<Arc<dyn CredentialResolver>>,
}
impl Boundary {
    pub async fn operation_catalog(
        &self,
        parts: Parts,
        vault: Option<uuid::Uuid>,
    ) -> Result<lapis_app_protocol::operations::OperationCatalog, Rejection> {
        self.run(parts, move |_, grants, bearer| {
            grants.operation_catalogs(bearer, vault).map_err(Into::into)
        })
        .await
    }
    pub async fn invoke_operation(
        &self,
        parts: Parts,
        vault: uuid::Uuid,
        plugin: String,
        id: String,
        request: lapis_app_protocol::operations::OperationRequest,
    ) -> Result<lapis_app_protocol::operations::OperationReply, Rejection> {
        self.run(parts, move |_, grants, bearer| {
            Ok(grants.invoke_operation(bearer, vault, &plugin, &id, request))
        })
        .await
    }
    pub fn new(grants: Arc<ContentGrants>, origin: &str) -> std::io::Result<Self> {
        Ok(Self {
            grants,
            policy: Arc::new(policy::Policy::new(origin)?),
            jobs: Arc::new(Semaphore::new(MAX_NATIVE_JOBS)),
            origin: origin.into(),
            resolver: None,
        })
    }
    pub fn with_resolver(mut self, resolver: Arc<dyn CredentialResolver>) -> Self {
        self.resolver = Some(resolver);
        self
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
            if let Some(resolver) = &boundary.resolver {
                let uri = parts
                    .extensions
                    .get::<axum::extract::OriginalUri>()
                    .map_or(&parts.uri, |original| &original.0);
                tokio::runtime::Handle::current()
                    .block_on(resolver.resolve(&bearer, uri.path()))?;
            }
            boundary.grants.check_credential(&bearer)?;
            action(parts, &boundary.grants, &bearer)
        })
        .await
    }
}
