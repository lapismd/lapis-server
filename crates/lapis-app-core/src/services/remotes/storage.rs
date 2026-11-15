use super::*;
use crate::vault::VaultRoot;
use cap_std::fs::MetadataExt as _;
use std::{future::Future, pin::Pin};
use std::{io, path::Path, sync::Mutex};

pub type RemoteCredentialFuture<'a, T = ()> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;
/// Backend-only custody commands; neither printable nor serializable.
pub enum RemoteCredentialAction {
    Save(RemoteCredentialInput),
    Forget(String),
    BeginSignIn,
    CheckSignIn,
    CancelSignIn,
}

/// Native-only custody. Renderers receive inventory, never resolved credentials.
pub trait RemoteCredentials: Send + Sync {
    fn inventory(&self) -> Result<RemoteSettings>;
    /// Implementations must enforce owner, endpoint and account scope again.
    fn resolve(&self, _connection: &RemoteConnection) -> Result<jj_vault::GitCredentials> {
        Err(unsupported())
    }
    fn resolve_async<'a>(
        &'a self,
        connection: &'a RemoteConnection,
    ) -> RemoteCredentialFuture<'a, jj_vault::GitCredentials> {
        Box::pin(async move { self.resolve(connection) })
    }
    fn control(&self, _action: RemoteCredentialAction) -> RemoteCredentialFuture<'_> {
        Box::pin(async { Err(unsupported()) })
    }
}
/// Installation selection only. No request can choose these native paths/providers.
pub struct RemoteStorage {
    pub(super) root: VaultRoot,
    pub(super) credentials: Arc<dyn RemoteCredentials>,
    pub(super) gate: Mutex<()>,
    pub(super) transfers: bool,
    pub(super) known_hosts: Option<std::path::PathBuf>,
}
impl RemoteStorage {
    pub fn new(directory: &Path, credentials: Arc<dyn RemoteCredentials>) -> Result<Self> {
        let root = VaultRoot::open(directory)?;
        if root.directory_handle()?.dir_metadata()?.mode() & 0o077 != 0 {
            return Err(Error::denied());
        }
        Ok(Self {
            root,
            credentials,
            gate: Mutex::new(()),
            transfers: false,
            known_hosts: None,
        })
    }
    /// Explicit host composition admits network commands. Saving alone does not.
    pub fn with_transfers(mut self, enabled: bool) -> Self {
        self.transfers = enabled;
        self
    }
    /// Host-selected trust file, outside every admitted vault; no DTO path.
    pub fn with_known_hosts(mut self, path: &Path) -> Result<Self> {
        let path = path.canonicalize()?;
        if !path.is_file() {
            return Err(Error::denied());
        }
        self.known_hosts = Some(path);
        Ok(self)
    }
    pub(in crate::services) fn validate_vault(&self, vault: &VaultRoot) -> Result<()> {
        self.root.verify_physical_root()?;
        if self.root.path().starts_with(vault.path())
            || vault.path().starts_with(self.root.path())
            || self
                .known_hosts
                .as_ref()
                .is_some_and(|path| path.starts_with(vault.path()))
        {
            return Err(Error::denied());
        }
        Ok(())
    }
    pub(super) fn selection(&self, vault: &super::VaultService) -> Result<Selection> {
        let state = vault.lock()?;
        state.root.verify_physical_root()?;
        self.validate_vault(&state.root)?;
        let directory = vault.id.to_string();
        match self.root.mkdir(&directory, false) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
        use std::os::unix::fs::PermissionsExt as _;
        let handle = self.root.open_directory(&directory)?.open(".")?.into_std();
        handle.set_permissions(std::fs::Permissions::from_mode(0o700))?;
        let private = VaultRoot::open(&self.root.path().join(directory))?;
        let selection = Selection::open(private, &state.root)?;
        self.root.verify_physical_root()?;
        Ok(selection)
    }
}
