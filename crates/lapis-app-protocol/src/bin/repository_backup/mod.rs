use lapis_app_protocol::repository_backup::*;
use ts_rs::{Config, TS};

pub(super) fn declarations(config: &Config) -> Vec<String> {
    vec![
        RepositoryBackupCommand::decl(config),
        RepositoryBackupPhase::decl(config),
        RepositoryBackupStatus::decl(config),
        RepositoryBackupSnapshot::decl(config),
    ]
}
