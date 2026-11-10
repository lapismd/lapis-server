//! Client-owned admission around Terminal Host's native PTY engine.
use super::{Client, Error, Result, VaultService};
use lapis_app_protocol::{
    ErrorCode,
    terminal::{TerminalCommand, TerminalOutput, TerminalValue},
};
use lapis_terminal_service::PtyService;
use std::{collections::HashMap, path::Path, sync::Arc, time::Duration};
use uuid::Uuid;

pub(super) type Owners = HashMap<Uuid, Arc<PtyService>>;
fn failure(error: std::io::Error) -> Error {
    let code = match error.kind() {
        std::io::ErrorKind::InvalidInput => ErrorCode::InvalidRequest,
        std::io::ErrorKind::NotFound => ErrorCode::NotFound,
        std::io::ErrorKind::WouldBlock => ErrorCode::Busy,
        std::io::ErrorKind::PermissionDenied => ErrorCode::PermissionDenied,
        _ => ErrorCode::Io,
    };
    Error::new(code, "Terminal operation failed")
}
impl Client {
    fn terminal_owner(&self, create: bool) -> Result<Arc<PtyService>> {
        // State then owner registry: detach fences first and drains outside state.
        let state = self.vault.lock()?;
        state.require_client(self.id)?;
        state.root.verify_physical_root()?;
        let mut owners = self.vault.terminals.lock().map_err(|_| Error::closed())?;
        if !owners.contains_key(&self.id) && create {
            if owners.len() >= 8 {
                return Err(Error::busy());
            }
            let owner =
                PtyService::new(state.root.path(), Path::new("/bin/bash")).map_err(failure)?;
            owners.insert(self.id, Arc::new(owner));
        }
        owners.get(&self.id).cloned().ok_or_else(Error::denied)
    }
    pub fn terminal(&self, command: TerminalCommand) -> Result<TerminalValue> {
        if matches!(command, TerminalCommand::CloseAll {}) {
            self.vault.lock()?.require_client(self.id)?;
            self.vault.revoke_terminals(Some(self.id))?;
            return Ok(TerminalValue::Done {});
        }
        let owner = self.terminal_owner(matches!(command, TerminalCommand::Create { .. }))?;
        let result = match command {
            TerminalCommand::Create { cwd, cols, rows } => TerminalValue::Created {
                session_id: owner.create(cwd.as_deref(), cols, rows).map_err(failure)?,
            },
            TerminalCommand::Write { session_id, data } => {
                owner.write(session_id, data.as_bytes()).map_err(failure)?;
                TerminalValue::Done {}
            }
            TerminalCommand::Resize {
                session_id,
                cols,
                rows,
            } => {
                owner.resize(session_id, cols, rows).map_err(failure)?;
                TerminalValue::Done {}
            }
            TerminalCommand::Stop { session_id } => {
                owner.stop(session_id).map_err(failure)?;
                TerminalValue::Done {}
            }
            TerminalCommand::CloseAll {} => unreachable!(),
        };
        self.vault.lock()?.require_client(self.id)?;
        Ok(result)
    }
    pub fn terminal_output(
        &self,
        session_id: Uuid,
        cursor: u64,
        timeout: Duration,
    ) -> Result<TerminalOutput> {
        let owner = self.terminal_owner(false)?;
        let output = owner
            .read(session_id, cursor, timeout.min(Duration::from_secs(1)))
            .map_err(failure)?;
        self.vault.lock()?.require_client(self.id)?;
        Ok(TerminalOutput {
            cursor: output.cursor,
            bytes: output.bytes,
            truncated: output.truncated,
            running: output.running,
            exit_code: output.exit_code,
        })
    }
}
impl VaultService {
    pub(super) fn revoke_terminals(&self, client: Option<Uuid>) -> Result<()> {
        let mut owners = self.terminals.lock().map_err(|_| Error::closed())?;
        let retired = match client {
            Some(id) => owners.remove(&id).into_iter().collect::<Vec<_>>(),
            None => owners.drain().map(|(_, owner)| owner).collect(),
        };
        drop(owners);
        let mut error = None;
        for owner in retired {
            if let Err(failure) = owner.close().map_err(failure) {
                error.get_or_insert(failure);
            }
        }
        error.map_or(Ok(()), Err)
    }
}
