use super::*;

pub(super) fn read(
    storage: &RemoteStorage,
    selection: SelectionState,
    original: Option<jj_store_jj::original_remotes::Snapshot>,
) -> Result<RepositoryRemotesSnapshot> {
    let mut settings = storage.credentials.inventory()?;
    let connection = selection.backup;
    let status = connection_status(connection.as_ref(), &settings);
    settings.automatic_sync = selection.automatic_sync;
    settings.targets = vec![RemoteRepositorySettings {
        target: RemoteRepositoryTarget::Backup,
        connection,
        status,
        bookmarks: vec![],
        remotes: None,
        // Transport admission arrives separately; a settings projection grants none.
        can_fetch: false,
        can_push: false,
        observation: None,
        review: None,
        publication: None,
    }];
    let mut targets = vec![RemoteRepositoryTarget::Backup];
    if let Some(original) = original {
        targets.push(RemoteRepositoryTarget::Original);
        let bound = selection
            .original_observed
            .as_ref()
            .is_some_and(|observed| selection.original_binding.as_ref() == Some(&observed.source))
            && selection.original.as_ref().is_some_and(|connection| {
                original
                    .remotes
                    .iter()
                    .any(|r| r.name == connection.name && r.url.as_ref() == Some(&connection.url))
            });
        let status = if bound {
            connection_status(selection.original.as_ref(), &settings)
        } else {
            RemoteConnectionState::Disconnected
        };
        settings.targets.push(RemoteRepositorySettings {
            target: RemoteRepositoryTarget::Original,
            connection: selection.original,
            status,
            bookmarks: original
                .bookmarks
                .into_iter()
                .filter(|b| !b.conflicted && b.commit.is_some())
                .map(|b| b.name)
                .collect(),
            remotes: Some(
                original
                    .remotes
                    .into_iter()
                    .map(|r| RemoteEndpoint {
                        name: r.name,
                        url: r.url,
                    })
                    .collect(),
            ),
            // Immutable discovery cannot authorize remote mutations.
            can_fetch: false,
            can_push: false,
            observation: None,
            review: None,
            publication: None,
        });
    }
    Ok(RepositoryRemotesSnapshot {
        revision: selection.revision,
        capabilities: RemoteCapabilities {
            targets,
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

fn connection_status(
    connection: Option<&RemoteConnection>,
    settings: &RemoteSettings,
) -> RemoteConnectionState {
    match connection {
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
    }
}
