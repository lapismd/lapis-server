use lapis_app_protocol::repository_restore::*;
use ts_rs::{Config, TS};

pub(super) fn declarations(config: &Config) -> Vec<String> {
    vec![
        RepositoryRestoreCommand::decl(config),
        RepositoryFileRestoreRequest::decl(config),
        RepositoryVaultRestoreRequest::decl(config),
        RepositoryFileRestorePreview::decl(config),
        RepositoryVaultRestorePreview::decl(config),
        RepositoryCheckpoint::decl(config),
        RepositoryCapture::decl(config),
        RepositoryRestoreValue::decl(config),
    ]
}
