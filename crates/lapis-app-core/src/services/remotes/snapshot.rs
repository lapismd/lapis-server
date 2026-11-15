use super::*;

pub(super) fn read(
    storage: &RemoteStorage,
    private: &Selection,
    selection: SelectionState,
    original: Option<original::Discovery>,
    vault: &VaultService,
    runtime: &tokio::runtime::Runtime,
) -> Result<RepositoryRemotesSnapshot> {
    let mut settings = storage.credentials.inventory()?;
    settings.automatic_sync = selection.automatic_sync;
    settings.targets = vec![backup::project(
        vault, storage, private, &selection, &settings, runtime,
    )?];
    let mut targets = vec![RemoteRepositoryTarget::Backup];
    if let Some(original) = original {
        let binding = original
            .repository
            .transport_binding(private.private_path())
            .map_err(transfer::original_error)?;
        let pending = operations::pending(&binding)?;
        let native = original.snapshot;
        targets.push(RemoteRepositoryTarget::Original);
        let bound = selection
            .original_observed
            .as_ref()
            .is_some_and(|observed| selection.original_binding.as_ref() == Some(&observed.source))
            && selection.original.as_ref().is_some_and(|connection| {
                native
                    .remotes
                    .iter()
                    .any(|r| r.name == connection.name && r.url.as_ref() == Some(&connection.url))
            });
        let status = if bound {
            connection_status(selection.original.as_ref(), &settings)
        } else {
            RemoteConnectionState::Disconnected
        };
        let admitted = storage.transfers
            && bound
            && matches!(
                status,
                RemoteConnectionState::Connected | RemoteConnectionState::ReadOnly
            );
        let journal = if bound {
            journal::Journal::open(private, &selection)?
        } else {
            None
        };
        let review = journal
            .as_ref()
            .and_then(|j| j.review.as_ref())
            .filter(|r| !r.started && pending.is_none())
            .map(journal::public)
            .transpose()?;
        let publication = pending.as_ref().map(|p| RemotePublication {
            id: p.request_id.clone(),
            commit: p.after.clone(),
            outcome: if p.outcome == jj_vault::PushOutcome::Stopped {
                RemotePublicationState::Stopped
            } else {
                RemotePublicationState::Unknown
            },
        });
        let can_push = admitted
            && pending.is_none()
            && matches!(status, RemoteConnectionState::Connected)
            && selection.original.as_ref().is_some_and(|c| {
                native
                    .bookmarks
                    .iter()
                    .any(|b| b.name == c.bookmark && b.commit.is_some() && !b.conflicted)
            });
        settings.targets.push(RemoteRepositorySettings {
            target: RemoteRepositoryTarget::Original,
            connection: selection.original,
            status,
            bookmarks: native
                .bookmarks
                .into_iter()
                .filter(|b| !b.conflicted && b.commit.is_some())
                .map(|b| b.name)
                .collect(),
            remotes: Some(
                native
                    .remotes
                    .into_iter()
                    .map(|r| RemoteEndpoint {
                        name: r.name,
                        url: r.url,
                    })
                    .collect(),
            ),
            can_fetch: admitted && pending.is_none(),
            can_push,
            observation: journal.and_then(|j| j.observation),
            review,
            publication,
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

pub(super) fn connection_status(
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
