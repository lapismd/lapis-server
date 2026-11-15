//! Owner-scoped secret custody stays native; every vault observes its epoch.
use super::*;
use crate::vault::{VaultRoot, WriteOptions};
use cap_std::fs::MetadataExt as _;
use std::{fs::File, io, os::unix::fs::MetadataExt as _};
const EPOCH: &str = ".credential-epoch.json";
const MAX: u64 = 9_007_199_254_740_991;
pub(super) fn lock(storage: &RemoteStorage) -> Result<File> {
    let root = &storage.root;
    root.verify_physical_root()?;
    let path = ".remotes-owner.lock";
    match root.create_file(path, &b""[..], &WriteOptions::default()) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    let metadata = root.entry_metadata(path)?;
    let file = root.directory_handle()?.open(path)?.into_std();
    let held = file.metadata()?;
    if !metadata.is_file() || metadata.dev() != held.dev() || metadata.ino() != held.ino() {
        return Err(Error::denied());
    }
    file.try_lock().map_err(|_| Error::busy())?;
    root.verify_physical_root()?;
    Ok(file)
}
pub(super) fn epoch(storage: &RemoteStorage) -> Result<u64> {
    let epoch = review_storage::load_root::<u64>(&storage.root, EPOCH)?.unwrap_or(0);
    if epoch > MAX {
        return Err(Error::denied());
    }
    Ok(epoch)
}
pub(super) fn is_command(command: &RepositoryRemotesCommand) -> bool {
    use RepositoryRemotesCommand as C;
    matches!(
        command,
        C::SaveCredential { .. }
            | C::ForgetCredential { .. }
            | C::BeginSignIn { .. }
            | C::CheckSignIn { .. }
            | C::CancelSignIn { .. }
    )
}
pub(super) fn run(
    storage: &RemoteStorage,
    selection: &Selection,
    command: RepositoryRemotesCommand,
    executor: &tokio::runtime::Runtime,
) -> Result<()> {
    use RepositoryRemotesCommand as C;
    let (expected, action) = match command {
        C::SaveCredential {
            expected_revision,
            input,
        } => (expected_revision, RemoteCredentialAction::Save(input)),
        C::ForgetCredential {
            expected_revision,
            id,
        } => (expected_revision, RemoteCredentialAction::Forget(id)),
        C::BeginSignIn { expected_revision } => {
            (expected_revision, RemoteCredentialAction::BeginSignIn)
        }
        C::CheckSignIn { expected_revision } => {
            (expected_revision, RemoteCredentialAction::CheckSignIn)
        }
        C::CancelSignIn { expected_revision } => {
            (expected_revision, RemoteCredentialAction::CancelSignIn)
        }
        _ => return Err(unsupported()),
    };
    {
        let (_guard, _) = selection.guard(expected)?;
        let settings = storage.credentials.inventory()?;
        preflight(&settings, &action)?;
        protect(storage, &action)?;
    }
    // Reserve before custody: a lost response or partial failure cannot let
    // another vault reuse a pre-mutation settings revision.
    let next = epoch(storage)?
        .checked_add(1)
        .filter(|v| *v <= MAX)
        .ok_or_else(Error::busy)?;
    review_storage::save_root(&storage.root, EPOCH, &next)?;
    let state = selection.observe_credentials(next)?;
    let (_guard, _) = selection.guard(state.revision)?;
    executor.block_on(storage.credentials.control(action))
}
pub(super) fn preflight(settings: &RemoteSettings, action: &RemoteCredentialAction) -> Result<()> {
    use RemoteCredentialAction as A;
    if matches!(action, A::Save(_) | A::Forget(_))
        && !matches!(settings.storage, RemoteStorageState::Available)
    {
        return Err(Error::new(
            ErrorCode::PermissionDenied,
            "Credential storage is locked or unavailable",
        ));
    }
    match action {
        A::Save(input) => {
            let origin = format!("https://{}", input.host);
            let url = reqwest::Url::parse(&origin).map_err(|_| invalid())?;
            if input.label.trim().is_empty()
                || input.label.len() > 128
                || input.label.chars().any(char::is_control)
                || input.host.len() > 256
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || url.origin().ascii_serialization().to_lowercase() != origin.to_lowercase()
                || input.username.is_empty()
                || input.username.len() > 128
                || !input
                    .username
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
                || input.token.is_empty()
                || input.token.len() > 4096
                || input.token.chars().any(char::is_control)
            {
                return Err(invalid());
            }
            if let Some(id) = &input.replace {
                let existing = settings
                    .credentials
                    .iter()
                    .find(|c| c.id == *id)
                    .ok_or_else(invalid)?;
                let raw = matches!(input.provider, RemoteCredentialProvider::Https);
                if !existing.host.eq_ignore_ascii_case(&input.host)
                    || existing.username != input.username
                    || raw != matches!(existing.method, RemoteCredentialMethod::Token)
                {
                    return Err(invalid());
                }
            }
        }
        A::Forget(id) if !settings.credentials.iter().any(|c| c.id == *id) => return Err(invalid()),
        A::BeginSignIn | A::CheckSignIn if !settings.device_sign_in_available => {
            return Err(unsupported());
        }
        _ => {}
    }
    Ok(())
}
fn invalid() -> Error {
    Error::new(
        ErrorCode::InvalidRequest,
        "Choose a valid scoped credential and account",
    )
}
fn protect(storage: &RemoteStorage, action: &RemoteCredentialAction) -> Result<()> {
    use RemoteCredentialAction as A;
    let selected = match action {
        A::Forget(id) => Some(id.as_str()),
        A::Save(input) if matches!(input.provider, RemoteCredentialProvider::Https) => {
            let Some(id) = input.replace.as_deref() else {
                return Ok(());
            };
            Some(id)
        }
        A::Save(_) | A::CheckSignIn => None,
        A::BeginSignIn | A::CancelSignIn => return Ok(()),
    };
    storage.root.verify_physical_root()?;
    for (index, entry) in storage.root.directory_handle()?.entries()?.enumerate() {
        if index >= 4096 {
            return Err(Error::busy());
        }
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| Error::denied())?;
        if name.starts_with('.') {
            continue;
        }
        let id = uuid::Uuid::parse_str(&name).map_err(|_| Error::denied())?;
        if id.to_string() != name || !entry.file_type()?.is_dir() {
            return Err(Error::denied());
        }
        let root = VaultRoot::open(&storage.root.path().join(name))?;
        for held in [
            journal::held_credential(&root)?,
            backup_journal::held_credential(&root)?,
        ]
        .into_iter()
        .flatten()
        {
            if selected.is_none_or(|id| id == held) {
                return Err(Error::new(
                    ErrorCode::Busy,
                    "Observe or stop the pending publication before changing its credentials",
                ));
            }
        }
    }
    storage.root.verify_physical_root()?;
    Ok(())
}
