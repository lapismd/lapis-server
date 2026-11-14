use lapis_app_protocol::operations::*;
use ts_rs::{Config, TS};

pub fn declarations(config: &Config) -> Vec<String> {
    vec![
        OperationArgument::decl(config),
        OperationHelp::decl(config),
        OperationDescriptor::decl(config),
        OperationCatalog::decl(config),
        OperationRequest::decl(config),
        OperationOutcome::decl(config),
        OperationReply::decl(config),
        OperationContribution::decl(config),
    ]
}

