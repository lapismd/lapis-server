use super::*;

pub(super) fn read(
    storage: &RemoteStorage,
    selection: SelectionState,
) -> Result<RepositoryRemotesSnapshot> {
    let mut settings = storage.credentials.inventory()?;
    let connection = selection.backup;
    let status = match &connection {
        None => RemoteConnectionState::Disconnected,
        Some(c) if c.authentication == RemoteAuthentication::Anonymous => {
            RemoteConnectionState::ReadOnly
        }
        Some(c)
            if matches!(
                c.authentication,
                RemoteAuthentication::SshAgent | RemoteAuthentication::SshDefault
            ) && settings.ssh_available =>
        {
            RemoteConnectionState::Connected
        }
        Some(c)
            if matches!(settings.storage, RemoteStorageState::Available)
                && settings
                    .credentials
                    .iter()
                    .any(|s| s.id == c.credential && s.available) =>
        {
            RemoteConnectionState::Connected
        }
        Some(_) => RemoteConnectionState::CredentialsRequired,
    };
    settings.automatic_sync = selection.automatic_sync;
    settings.targets = vec![RemoteRepositorySettings {
        target: RemoteRepositoryTarget::Backup,
        connection,
        status,
        bookmarks: vec![],
        // Transport admission arrives separately; a settings projection grants none.
        can_fetch: false,
        can_push: false,
        observation: None,
        review: None,
        publication: None,
    }];
    Ok(RepositoryRemotesSnapshot {
        revision: selection.revision,
        capabilities: RemoteCapabilities {
            targets: vec![RemoteRepositoryTarget::Backup],
            anonymous_https: true,
            https_tokens: matches!(settings.storage, RemoteStorageState::Available),
            host_ssh_agent: settings.ssh_available,
            host_ssh_default: settings.ssh_available,
            github_device_sign_in: settings.device_sign_in_available,
            forgejo_tokens: matches!(settings.storage, RemoteStorageState::Available),
        },
        settings,
    })
}
