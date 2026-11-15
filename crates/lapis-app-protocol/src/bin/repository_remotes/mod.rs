use lapis_app_protocol::repository_remotes::*;
use ts_rs::{Config, TS};
pub(super) fn declarations(config: &Config) -> Vec<String> {
    vec![
        RemoteHost::decl(config),
        RemoteStorageState::decl(config),
        RemoteConnectionState::decl(config),
        RemoteCredentialMethod::decl(config),
        RemoteCredentialProvider::decl(config),
        RemotePublicationState::decl(config),
        RemoteAuthorizationState::decl(config),
        RemoteRepositoryTarget::decl(config),
        RemoteAuthentication::decl(config),
        RemoteConnection::decl(config),
        RemoteEndpoint::decl(config),
        RemoteCapabilities::decl(config),
        RemoteCredentialSummary::decl(config),
        RemotePushReview::decl(config),
        RemoteObservation::decl(config),
        RemotePublication::decl(config),
        RemoteRepositorySettings::decl(config),
        RemoteCredentialInput::decl(config),
        RepositoryRemotesCommand::decl(config),
        RemoteAuthorization::decl(config),
        RemoteSettings::decl(config),
        RepositoryRemotesSnapshot::decl(config),
    ]
}
