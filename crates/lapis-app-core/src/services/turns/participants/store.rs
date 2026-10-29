use super::{ParticipantMessage, ParticipantState, invalid};
use crate::{
    record_store::RecordStore,
    services::{Result, turns::TurnStorage},
    turns::TurnJournal,
    vault::{VaultRoot, sync_directory},
};
use lapis_app_protocol::turns::TurnPhase;
use uuid::Uuid;

pub(in crate::services::turns) struct Messages {
    pub store: RecordStore<ParticipantMessage>,
}
impl Messages {
    pub(super) fn open(
        storage: &TurnStorage,
        vault: Uuid,
        content: &VaultRoot,
        journal: &TurnJournal,
    ) -> Result<Self> {
        storage.root.verify_physical_root()?;
        match storage.root.mkdir("participants", false) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }
        sync_directory(&storage.root.directory_handle()?)?;
        let profile = storage.root.open_child("participants")?;
        let store: RecordStore<ParticipantMessage> =
            RecordStore::open(profile.path(), vault, content)?;
        for id in store.ids()? {
            let (mut message, expected) = store.read(id)?.ok_or_else(super::super::failure)?;
            for (index, delivery) in message.deliveries.iter_mut().enumerate() {
                let record = journal.read(delivery.request_id)?;
                if record.as_ref().is_some_and(|r| {
                    r.input != message.input.turns[index]
                        || r.conversation_id != message.conversation_id
                }) {
                    return Err(invalid("Participant journal association changed"));
                }
                if !delivery.state.terminal() {
                    delivery.state = match record.as_ref().map(|r| r.phase) {
                        Some(TurnPhase::Completed) => ParticipantState::Completed,
                        Some(TurnPhase::Failed) => ParticipantState::Rejected,
                        Some(TurnPhase::Unknown | TurnPhase::Running | TurnPhase::Cancelled) => {
                            ParticipantState::Unknown
                        }
                        _ if delivery.state == ParticipantState::Started => {
                            ParticipantState::Unknown
                        }
                        _ => ParticipantState::NotSent,
                    };
                    delivery.outcome_published = delivery.state == ParticipantState::Completed;
                }
            }
            message.recovered = true;
            store.write(&message, Some(&expected))?;
        }
        Ok(Self { store })
    }
}
pub(super) fn open(
    data: &mut super::super::Data,
    storage: &TurnStorage,
    work: &crate::services::Work,
) -> Result<()> {
    if data.participants.is_none() {
        let root = work.vault.lock()?.root.try_clone()?;
        data.participants = Some(Messages::open(
            storage,
            work.vault.id,
            &root,
            &data.journal,
        )?);
    }
    Ok(())
}
