use super::super::{Result, TurnStorage, Work};
use super::{QueuePhase, TurnQueue, receipt};
use crate::{
    record_store::RecordStore,
    vault::{VaultRoot, sync_directory},
};
use std::sync::Arc;
use uuid::Uuid;

pub(in crate::services::turns) struct Queues {
    pub store: RecordStore<TurnQueue>,
    pub live: Option<(Uuid, Arc<Work>)>,
}
impl Queues {
    pub fn open(
        storage: &TurnStorage,
        vault: Uuid,
        content: &VaultRoot,
        journal: &crate::turns::TurnJournal,
    ) -> Result<Self> {
        storage.root.verify_physical_root()?;
        match storage.root.mkdir("queues", false) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }
        sync_directory(&storage.root.directory_handle()?)?;
        let profile = storage.root.open_child("queues")?;
        let store: RecordStore<TurnQueue> = RecordStore::open(profile.path(), vault, content)?;
        for id in store.ids()? {
            let (mut queue, bytes) = store.read(id)?.ok_or_else(super::super::failure)?;
            queue.paused = true;
            queue.barrier = Some("service_restarted".into());
            for entry in &mut queue.entries {
                if entry.phase == QueuePhase::Started {
                    // A durable terminal journal survives a lost queue acknowledgement.
                    // Accepted/Running have already been conservatively retired by open.
                    entry.phase = match journal.read(entry.request_id)? {
                        Some(record)
                            if record.conversation_id == queue.conversation_id
                                && record.input.scope_dir == queue.scope_dir
                                && record.input.target_id == entry.input.target_id =>
                        {
                            use lapis_app_protocol::turns::TurnPhase;
                            match record.phase {
                                TurnPhase::Completed => QueuePhase::Completed,
                                TurnPhase::Cancelled => QueuePhase::Cancelled,
                                TurnPhase::Failed => QueuePhase::Rejected,
                                _ => QueuePhase::Unknown,
                            }
                        }
                        _ => QueuePhase::Unknown,
                    };
                    entry.receipt = Some(receipt(entry)?);
                }
            }
            store.write(&queue, Some(&bytes))?;
        }
        Ok(Self { store, live: None })
    }
    pub fn pause(&mut self, reason: &str) -> Result<()> {
        if let Some((id, _)) = &self.live {
            let (mut queue, bytes) = self.store.read(*id)?.ok_or_else(super::super::failure)?;
            queue.paused = true;
            queue.barrier = Some(reason.into());
            self.store.write(&queue, Some(&bytes))?;
        }
        self.live = None;
        Ok(())
    }
    pub fn blocks(&self, input: &lapis_app_protocol::turns::TurnInput) -> Result<bool> {
        if self.live.is_some() {
            return Ok(true);
        }
        let Some(cursor) = &input.continuation else {
            return Ok(false);
        };
        Ok(self
            .store
            .read(cursor.conversation_id)?
            .is_some_and(|(q, _)| super::has_pending(&q)))
    }
}
