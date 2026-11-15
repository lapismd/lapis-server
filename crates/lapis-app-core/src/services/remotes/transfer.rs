//! Transport is admitted only by native composition and exact saved selection.
use super::*;
use jj_vault::{Binding, HostSshIdentity, RemoteTarget, RemoteTransfer};

pub(super) async fn connect(
    storage: &RemoteStorage,
    binding: Binding,
    connection: &RemoteConnection,
) -> Result<RemoteTransfer> {
    if !storage.transfers {
        return Err(unsupported());
    }
    admit_connection(storage, connection)?;
    let target = RemoteTarget {
        url: connection.url.clone(),
        branch: connection.branch.clone(),
    };
    use RemoteAuthentication as Auth;
    let transfer = match connection.authentication {
        Auth::Anonymous => RemoteTransfer::connect_anonymous(binding, target).await,
        Auth::SshAgent | Auth::SshDefault => {
            let identity = if connection.authentication == Auth::SshAgent {
                HostSshIdentity::Agent
            } else {
                HostSshIdentity::Default
            };
            RemoteTransfer::connect_ssh(binding, target, identity).await
        }
        Auth::Token | Auth::Forge => {
            let credential = storage.credentials.resolve(connection)?;
            RemoteTransfer::connect(binding, target, credential).await
        }
    }
    .map_err(|_| failed())?;
    if matches!(connection.authentication, Auth::SshAgent | Auth::SshDefault)
        && let Some(path) = &storage.known_hosts
    {
        return transfer.with_known_hosts(path).map_err(|_| failed());
    }
    Ok(transfer)
}
pub(super) fn failed() -> Error {
    Error::new(
        ErrorCode::Io,
        "Repository transfer failed; check any pending publication before retrying",
    )
}
pub(super) fn original_error(error: jj_store_jj::original_remotes::Error) -> Error {
    use jj_store_jj::original_remotes::Error as Native;
    match error {
        Native::Stale | Native::Diverged => Error::new(
            ErrorCode::Conflict,
            "Repository changed or diverged; fetch and review before publishing",
        ),
        Native::Denied => Error::denied(),
        Native::Binding | Native::Unsupported | Native::Limit => unsupported(),
        Native::Native | Native::Transport => failed(),
    }
}
