use super::{Host, Principal, Result};
use lapis_app_protocol::terminal::{TerminalCommand, TerminalOutput, TerminalValue};
use std::time::Duration;
use uuid::Uuid;
impl Host {
    pub fn terminal(
        &self,
        id: Uuid,
        principal: Principal,
        command: TerminalCommand,
    ) -> Result<TerminalValue> {
        let value = self
            .client(id, principal)?
            .terminal(command)
            .map_err(|error| error.0)?;
        self.client(id, principal)?;
        Ok(value)
    }
    pub fn terminal_output(
        &self,
        id: Uuid,
        principal: Principal,
        session: Uuid,
        cursor: u64,
    ) -> Result<TerminalOutput> {
        let value = self
            .client(id, principal)?
            .terminal_output(session, cursor, Duration::from_secs(1))
            .map_err(|error| error.0)?;
        self.client(id, principal)?;
        Ok(value)
    }
}
