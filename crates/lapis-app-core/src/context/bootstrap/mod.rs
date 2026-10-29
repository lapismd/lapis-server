// SPDX-License-Identifier: Apache-2.0
//! Retained bootstrap construction; readers never convey new vault authority.
mod format;
mod reader;
mod rules;
mod types;
use crate::services::{Error, Result};
use lapis_app_protocol::ErrorCode;
pub use types::*;

pub type BootstrapReader<'a> = dyn FnMut(&str) -> Result<Option<String>> + 'a;

pub fn prepare_bootstrap(
    input: &BootstrapInput,
    reader: Option<&mut BootstrapReader<'_>>,
    is_cancelled: impl Fn() -> bool,
) -> Result<PreparedBootstrap> {
    cancelled(&is_cancelled)?;
    let folders = match reader {
        Some(reader) => reader::collect(&input.scope_dir, reader, &is_cancelled)?,
        None => Vec::new(),
    };
    cancelled(&is_cancelled)?;
    let bootstrap = AgentBootstrap {
        application: BootstrapApplication {
            name: "Lapis Notes".into(),
        },
        conversation: BootstrapConversation {
            id: input.conversation_id.clone(),
            scope_dir: input.scope_dir.clone(),
            launch_note_path: input.launch_note_path.clone(),
        },
        workspace: input
            .workspace_label
            .as_ref()
            .filter(|label| !label.is_empty())
            .map(|label| BootstrapWorkspace {
                label: label.clone(),
            }),
        tools: input.tools.clone(),
        skills: input.skills.clone(),
        truncated: folders.iter().any(|entry| entry.truncated),
        folder_instructions: folders,
        instructions: rules::instructions(),
    };
    let text = format::render(&bootstrap)?;
    cancelled(&is_cancelled)?;
    Ok(PreparedBootstrap { bootstrap, text })
}

fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::InvalidRequest, message)
}
fn cancelled(check: &impl Fn() -> bool) -> Result<()> {
    if check() {
        return Err(Error::new(ErrorCode::Closed, "This operation was aborted"));
    }
    Ok(())
}
