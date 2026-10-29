use lapis_app_protocol::repository_sources::*;
use ts_rs::{Config, TS};

pub(super) fn declarations(config: &Config) -> Vec<String> {
    vec![
        RepositorySourcesCommand::decl(config),
        RepositorySources::decl(config),
    ]
}
