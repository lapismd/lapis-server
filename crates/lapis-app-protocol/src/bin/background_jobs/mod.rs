use lapis_plugin_jobs::*;
use ts_rs::{Config, TS};

pub(super) fn declarations(config: &Config) -> Vec<String> {
    vec![
        Schedule::decl(config),
        JobPermissions::decl(config),
        JobDescriptor::decl(config),
        JobConfiguration::decl(config),
        JobPhase::decl(config),
        JobRun::decl(config),
        JobStatus::decl(config),
        JobTransfer::decl(config),
        JobCommand::decl(config),
        JobSnapshot::decl(config),
    ]
}

pub(super) fn typescript(config: &Config) -> String {
    format!(
        "// Generated from lapis-app-protocol. Do not edit.\n{}\n",
        declarations(config)
            .iter()
            .map(|item| format!("export {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    )
}
