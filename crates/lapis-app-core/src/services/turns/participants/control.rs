use super::{ParticipantState, invalid, store};
use crate::services::{Client, Error, Result};
use lapis_app_protocol::ErrorCode;
use std::sync::atomic::Ordering;
use uuid::Uuid;

impl Client {
    pub(super) fn control_participant(
        &self,
        acceptance: Uuid,
        request: Option<Uuid>,
    ) -> Result<bool> {
        if acceptance.get_version_num() != 4 || request.is_some_and(|id| id.get_version_num() != 4)
        {
            return Err(invalid("Participant control identity is invalid"));
        }
        let (owner, work) = self.turn_owner()?;
        let mut data = owner.data()?;
        store::open(&mut data, &owner.storage, &work)?;
        let message = data
            .participants
            .as_ref()
            .expect("opened")
            .store
            .read(acceptance)?
            .ok_or_else(|| Error::new(ErrorCode::NotFound, "Participant message is unavailable"))?
            .0;
        if request.is_some_and(|id| !message.deliveries.iter().any(|d| d.request_id == id)) {
            return Err(invalid(
                "Native request does not belong to this participant acceptance",
            ));
        }
        let mut accepted = false;
        for delivery in message.deliveries {
            if !matches!(
                delivery.state,
                ParticipantState::Pending | ParticipantState::Started
            ) || request.is_some_and(|id| id != delivery.request_id)
            {
                continue;
            }
            if let Some(job) = data.jobs.get(&delivery.request_id)
                && job
                    .participant
                    .as_ref()
                    .is_some_and(|p| p.message == acceptance)
                && !job.done.load(Ordering::Acquire)
            {
                job.cancel.send_replace(true);
                accepted = true;
            }
        }
        if accepted {
            data.queues.pause("native_participant_cancelled")?;
        }
        Ok(accepted)
    }
}
