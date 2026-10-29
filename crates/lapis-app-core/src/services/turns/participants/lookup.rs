use super::{ParticipantMessage, invalid, store};
use crate::{
    record_store::StoredRecord,
    services::{Client, Result},
};
use uuid::Uuid;

impl Client {
    /// Discover existing authority only. Opening retains the existing conservative
    /// cold projection; this query never publishes portable bytes or claims work.
    pub fn participant_for_turn(&self, request_id: Uuid) -> Result<Option<ParticipantMessage>> {
        if request_id.get_version_num() != 4 {
            return Err(invalid("Native participant request identity is invalid"));
        }
        let (owner, work) = self.turn_owner()?;
        let mut data = owner.data()?;
        store::open(&mut data, &owner.storage, &work)?;
        let receipts = &data.participants.as_ref().expect("opened").store;
        let ids = receipts.ids()?;
        if ids.len() > ParticipantMessage::MAX_RECORDS {
            return Err(invalid("Native participant receipt limit exceeded"));
        }
        let mut found = None;
        for id in ids {
            let (message, _) = receipts
                .read(id)?
                .ok_or_else(|| invalid("Native participant receipt disappeared"))?;
            if let Some(index) = message
                .deliveries
                .iter()
                .position(|d| d.request_id == request_id)
            {
                let record = data
                    .journal
                    .read(request_id)?
                    .ok_or_else(|| invalid("Native participant journal association disappeared"))?;
                if record.conversation_id != message.conversation_id
                    || record.input != message.input.turns[index]
                {
                    return Err(invalid("Native participant journal association changed"));
                }
                if found.is_some() {
                    return Err(invalid(
                        "Native participant request association is ambiguous",
                    ));
                }
                found = Some(message);
            }
        }
        Ok(found)
    }
}
