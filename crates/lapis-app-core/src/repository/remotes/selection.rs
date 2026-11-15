//! Private connection references are independent of repository config and secret storage.
use crate::vault::{VaultRoot, WriteOptions};
use cap_std::fs::MetadataExt as _;
use lapis_app_protocol::repository_remotes::{
    RemoteAuthentication, RemoteConnection, RemoteRepositoryTarget as Target,
};
use serde::{Deserialize, Serialize};
use std::{fs::File, io, os::unix::fs::MetadataExt as _};

mod validation;
use validation::validate;

const PATH: &str = "repository-remotes.json";
const MAX_REVISION: u64 = 9_007_199_254_740_991;
const MAX_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OriginalObservation {
    pub source: String,
    pub configuration: String,
}
impl OriginalObservation {
    fn valid(&self) -> bool {
        digest(&self.source) && digest(&self.configuration)
    }
}
fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectionState {
    version: u8,
    pub revision: u64,
    pub backup: Option<RemoteConnection>,
    pub original: Option<RemoteConnection>,
    pub automatic_sync: bool,
    pub migrated: bool,
    #[serde(default)]
    pub original_observed: Option<OriginalObservation>,
    #[serde(default)]
    pub original_binding: Option<String>,
}
impl Default for SelectionState {
    fn default() -> Self {
        Self {
            version: 1,
            revision: 0,
            backup: None,
            original: None,
            automatic_sync: false,
            migrated: false,
            original_observed: None,
            original_binding: None,
        }
    }
}
impl SelectionState {
    pub fn connection(&self, target: Target) -> Option<&RemoteConnection> {
        match target {
            Target::Backup => self.backup.as_ref(),
            Target::Original => self.original.as_ref(),
        }
    }
    pub fn expect(&self, revision: u64) -> io::Result<()> {
        if self.revision != revision {
            return Err(io::Error::other(SelectionConflict));
        }
        Ok(())
    }
}
pub struct Selection {
    private: VaultRoot,
}
#[derive(Debug)]
pub(crate) struct SelectionConflict;
impl std::fmt::Display for SelectionConflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Repository connection changed; refresh before continuing")
    }
}
impl std::error::Error for SelectionConflict {}
fn invalid(message: &'static str) -> io::Error {
    io::Error::other(message)
}
impl Selection {
    /// Native construction only. No command DTO may select this directory.
    pub fn open(private: VaultRoot, vault: &VaultRoot) -> io::Result<Self> {
        private.verify_physical_root()?;
        vault.verify_physical_root()?;
        if private.path().starts_with(vault.path())
            || vault.path().starts_with(private.path())
            || private.physical_identity()? == vault.physical_identity()?
            || private.directory_handle()?.dir_metadata()?.mode() & 0o077 != 0
        {
            return Err(invalid(
                "Repository selection requires private installation storage",
            ));
        }
        Ok(Self { private })
    }
    fn loaded(&self) -> io::Result<(SelectionState, Option<Vec<u8>>)> {
        self.private.verify_physical_root()?;
        let metadata = match self.private.entry_metadata(PATH) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Ok((SelectionState::default(), None));
            }
            value => value?,
        };
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || metadata.len() > MAX_BYTES as u64
        {
            return Err(invalid("Saved repository selection is invalid"));
        }
        let bytes = self.private.read_range(PATH, 0, MAX_BYTES + 1)?;
        if bytes.len() > MAX_BYTES {
            return Err(invalid("Saved repository selection exceeds its limit"));
        }
        let state: SelectionState = serde_json::from_slice(&bytes)
            .map_err(|_| invalid("Saved repository selection is invalid"))?;
        if state.version != 1 || state.revision > MAX_REVISION {
            return Err(invalid("Saved repository selection version is unsupported"));
        }
        for (target, connection) in [
            (Target::Backup, &state.backup),
            (Target::Original, &state.original),
        ] {
            if let Some(connection) = connection {
                validate(target, connection)?;
            }
        }
        if state.original_observed.as_ref().is_some_and(|o| !o.valid())
            || state
                .original_binding
                .as_ref()
                .is_some_and(|s| !digest(s) || state.original.is_none())
        {
            return Err(invalid("Original source observation is invalid"));
        }
        if state.automatic_sync && state.backup.is_none() {
            return Err(invalid(
                "Automatic synchronization has no backup connection",
            ));
        }
        Ok((state, Some(bytes)))
    }
    pub fn read(&self) -> io::Result<SelectionState> {
        self.loaded().map(|v| v.0)
    }
    fn lock(&self) -> io::Result<File> {
        match self
            .private
            .create_file(".remotes.lock", &b""[..], &WriteOptions::default())
        {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
        let metadata = self.private.entry_metadata(".remotes.lock")?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(invalid("Repository selection lock changed"));
        }
        let file = self
            .private
            .directory_handle()?
            .open(".remotes.lock")?
            .into_std();
        if (metadata.dev(), metadata.ino()) != (file.metadata()?.dev(), file.metadata()?.ino()) {
            return Err(invalid("Repository selection lock changed"));
        }
        file.try_lock()
            .map_err(|_| invalid("Another repository selection is being saved"))?;
        Ok(file)
    }
    fn update(
        &self,
        expected: u64,
        apply: impl FnOnce(&mut SelectionState) -> io::Result<()>,
    ) -> io::Result<SelectionState> {
        let _lock = self.lock()?;
        let (mut state, bytes) = self.loaded()?;
        state.expect(expected)?;
        let revision = state
            .revision
            .checked_add(1)
            .filter(|v| *v <= MAX_REVISION)
            .ok_or_else(|| invalid("Repository selection revision is exhausted"))?;
        apply(&mut state)?;
        state.revision = revision;
        let next = serde_json::to_vec(&state)
            .map_err(|_| invalid("Repository selection could not be encoded"))?;
        if next.len() > MAX_BYTES {
            return Err(invalid("Repository selection exceeds its limit"));
        }
        self.private.write_if_unchanged(
            PATH,
            &next[..],
            bytes.as_deref(),
            &WriteOptions::default(),
        )?;
        self.private.verify_physical_root()?;
        Ok(state)
    }
    /// Caller proves target admission, credential scope and pending native publication first.
    /// This local metadata operation cannot perform a transfer or enable synchronization.
    pub fn configure(
        &self,
        expected: u64,
        target: Target,
        mut connection: RemoteConnection,
        pending: impl FnOnce() -> io::Result<bool>,
    ) -> io::Result<SelectionState> {
        if target == Target::Backup && connection.bookmark.is_empty() {
            connection.bookmark = "lapis-backup".into();
        }
        validate(target, &connection)?;
        self.update(expected, move |state| {
            if pending()? {
                return Err(invalid(
                    "Observe or stop the earlier publication before changing this connection",
                ));
            }
            match target {
                Target::Backup => {
                    if state.backup.as_ref() != Some(&connection) {
                        state.automatic_sync = false;
                    }
                    state.backup = Some(connection);
                }
                Target::Original => state.original = Some(connection),
            }
            state.migrated = true;
            Ok(())
        })
    }
    /// Private discovery fencing; it does not choose a connection or touch Git.
    pub fn observe_original(
        &self,
        observed: Option<OriginalObservation>,
    ) -> io::Result<SelectionState> {
        if observed.as_ref().is_some_and(|o| !o.valid()) {
            return Err(invalid("Original source observation is invalid"));
        }
        let current = self.read()?;
        if current.original_observed == observed {
            return Ok(current);
        }
        self.update(current.revision, move |state| {
            state.original_observed = observed;
            Ok(())
        })
    }
    pub(crate) fn private_path(&self) -> &std::path::Path {
        self.private.path()
    }
    /// Hold across an accepted transfer/review so another process cannot rebind
    /// either target while credentials or publication identities are in use.
    pub(crate) fn guard(&self, expected: u64) -> io::Result<(File, SelectionState)> {
        let lock = self.lock()?;
        let state = self.read()?;
        state.expect(expected)?;
        Ok((lock, state))
    }
    /// The native config transaction is explicit and runs under the private
    /// selection lock. If the final private write fails, endpoint/source checks
    /// keep the previous selection from authorizing the changed configuration.
    pub fn configure_original(
        &self,
        expected: u64,
        connection: RemoteConnection,
        observed: OriginalObservation,
        pending: impl FnOnce() -> io::Result<bool>,
        save: impl FnOnce() -> io::Result<OriginalObservation>,
    ) -> io::Result<SelectionState> {
        validate(Target::Original, &connection)?;
        if !observed.valid() {
            return Err(invalid("Original source observation is invalid"));
        }
        self.update(expected, move |state| {
            if state.original_observed.as_ref() != Some(&observed) {
                return Err(io::Error::other(SelectionConflict));
            }
            if pending()? {
                return Err(invalid(
                    "Observe or stop the earlier publication before changing this connection",
                ));
            }
            let saved = save()?;
            if !saved.valid() || saved.source != observed.source {
                return Err(invalid("Original source changed while saving"));
            }
            state.original_binding = Some(saved.source.clone());
            state.original_observed = Some(saved);
            state.original = Some(connection);
            state.migrated = true;
            Ok(())
        })
    }
    pub fn set_automatic(&self, expected: u64, enabled: bool) -> io::Result<SelectionState> {
        self.update(expected, |state| {
            if enabled
                && state
                    .backup
                    .as_ref()
                    .is_none_or(|c| c.authentication == RemoteAuthentication::Anonymous)
            {
                return Err(invalid(
                    "Choose an authenticated backup connection before enabling synchronization",
                ));
            }
            state.automatic_sync = enabled;
            Ok(())
        })
    }
    /// Host startup supplies an existing native selection and saved preference.
    /// Reopening cannot overwrite a newer connection or turn an absent preference on.
    pub fn import_legacy(
        &self,
        expected: u64,
        connection: Option<RemoteConnection>,
        automatic: Option<bool>,
    ) -> io::Result<SelectionState> {
        let current = self.read()?;
        if current.migrated {
            return Ok(current);
        }
        if let Some(connection) = &connection {
            validate(Target::Backup, connection)?;
        }
        self.update(expected, move |state| {
            if state.migrated {
                return Err(invalid("Repository selection was already migrated"));
            }
            state.automatic_sync = connection
                .as_ref()
                .is_some_and(|c| c.authentication != RemoteAuthentication::Anonymous)
                && automatic == Some(true);
            state.backup = connection;
            state.migrated = true;
            Ok(())
        })
    }
    /// Reserve a new observation epoch before a separately owned credential mutation.
    pub fn advance(&self, expected: u64) -> io::Result<SelectionState> {
        self.update(expected, |_| Ok(()))
    }
}
