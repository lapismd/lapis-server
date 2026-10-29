use crate::{
    services::{Error, Result},
    vault::VaultRoot,
};
use lapis_ai_service::{
    ExecutionBackend, HostExecution, Launch, SessionBinding, runtime::SessionProcessor,
};
use lapis_app_protocol::ErrorCode;
use std::{path::Path, sync::Arc};
use uuid::Uuid;

/// Installation policy only: browser DTOs cannot construct a prepared launch.
pub trait AgentPolicy: Send + Sync + 'static {
    fn targets(
        &self,
        _vault: Uuid,
        _scope: &VaultRoot,
    ) -> lapis_ai_service::Result<Vec<lapis_app_protocol::turns::TurnTarget>> {
        Ok(Vec::new())
    }
    /// Trusted declared data only; default-empty keeps existing native hosts compatible.
    fn participant_profiles(
        &self,
        _vault: Uuid,
        _scope: &VaultRoot,
    ) -> lapis_ai_service::Result<Vec<(String, lapis_app_protocol::queues::QueueSelection)>> {
        Ok(Vec::new())
    }
    fn resolve(
        &self,
        vault: Uuid,
        scope: &VaultRoot,
        target: &str,
    ) -> lapis_ai_service::Result<Launch>;
    fn processor(
        &self,
        _vault: Uuid,
        _binding: &SessionBinding,
    ) -> lapis_ai_service::Result<Option<Box<dyn SessionProcessor>>> {
        Ok(None)
    }
}
pub struct TurnStorage {
    pub(super) root: VaultRoot,
    pub(super) policy: Arc<dyn AgentPolicy>,
    pub(super) runtime: tokio::runtime::Handle,
    pub(super) backend: Arc<dyn ExecutionBackend>,
}
impl TurnStorage {
    pub fn new(
        directory: &Path,
        policy: Arc<dyn AgentPolicy>,
        runtime: tokio::runtime::Handle,
    ) -> Result<Self> {
        if !directory.is_absolute() || !std::fs::symlink_metadata(directory)?.is_dir() {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "Native turn storage requires a physical profile",
            ));
        }
        Ok(Self {
            root: VaultRoot::open(directory)?,
            policy,
            runtime,
            backend: Arc::new(HostExecution),
        })
    }
    /// A native host may select another supervised backend; never from a request.
    pub fn with_backend(mut self, backend: Arc<dyn ExecutionBackend>) -> Self {
        self.backend = backend;
        self
    }
    pub(crate) fn validate_vault(&self, root: &VaultRoot) -> Result<()> {
        self.root.verify_physical_root()?;
        root.verify_physical_root()?;
        if self.root.path().starts_with(root.path()) || root.path().starts_with(self.root.path()) {
            return Err(Error::new(
                ErrorCode::PermissionDenied,
                "Native turn authority cannot be vault content",
            ));
        }
        Ok(())
    }
}
