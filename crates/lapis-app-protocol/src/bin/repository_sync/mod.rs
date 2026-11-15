use lapis_app_protocol::repository_sync::*;
use ts_rs::{Config, TS};
pub(super) fn declarations(config: &Config) -> Vec<String> {
    vec![
        RepositorySyncCommand::decl(config),
        RepositorySyncResolution::decl(config),
        RepositorySyncPhase::decl(config),
        RepositorySyncProposal::decl(config),
        RepositorySyncTerm::decl(config),
        RepositorySyncConflict::decl(config),
        RepositorySyncSnapshot::decl(config),
    ]
}
