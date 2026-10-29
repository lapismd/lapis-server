use super::{Client, Error, Result};
use lapis_app_protocol::{
    ErrorCode,
    turns::{TurnCommand, TurnPhase, TurnValue},
};
use std::sync::atomic::Ordering;

impl Client {
    /// Hosts already hold a client admitted to one vault and generation.
    /// Durable acceptance belongs to the native task, never to this reply.
    pub fn call_turn(&self, command: TurnCommand) -> Result<TurnValue> {
        match command {
            TurnCommand::Targets {} => {
                let (owner, work) = self.turn_owner()?;
                let data = owner.data()?;
                owner.require_enabled(&data)?;
                drop(data);
                let root = work.vault.lock()?.root.try_clone()?;
                let targets = owner
                    .storage
                    .policy
                    .targets(work.vault.id, &root)
                    .map_err(|_| {
                        Error::new(ErrorCode::Io, "Native agent catalogue is unavailable")
                    })?;
                Ok(TurnValue::Targets { targets })
            }
            TurnCommand::SetEnabled { enabled } => Ok(TurnValue::Availability {
                enabled: self.set_turns_enabled(enabled)?,
            }),
            TurnCommand::Submit { input } => Ok(TurnValue::Record {
                record: Some(Box::new(self.submit_turn(*input)?)),
            }),
            TurnCommand::Read { request_id } => {
                id(request_id)?;
                Ok(TurnValue::Record {
                    record: self.turn(request_id)?.map(Box::new),
                })
            }
            TurnCommand::List {} => Ok(TurnValue::Records {
                records: self.turns()?,
            }),
            TurnCommand::Cancel { request_id } => {
                id(request_id)?;
                let (owner, _work) = self.turn_owner()?;
                let mut data = owner.data()?;
                let Some(record) = data.journal.read(request_id)? else {
                    return Err(Error::new(
                        ErrorCode::NotFound,
                        "Native turn is unavailable",
                    ));
                };
                if !matches!(record.phase, TurnPhase::Accepted | TurnPhase::Running) {
                    return Ok(TurnValue::CancelRequested { accepted: false });
                }
                data.queues.pause("native_turn_cancelled")?;
                let accepted = data.jobs.get(&request_id).is_some_and(|job| {
                    if job.done.load(Ordering::Acquire) {
                        return false;
                    }
                    job.cancel.send_replace(true);
                    true
                });
                Ok(TurnValue::CancelRequested { accepted })
            }
        }
    }
}
fn id(value: uuid::Uuid) -> Result<()> {
    if value.get_version_num() == 4 {
        Ok(())
    } else {
        Err(Error::new(
            ErrorCode::InvalidRequest,
            "Native request identity is invalid",
        ))
    }
}
