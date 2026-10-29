use super::{
    Database, Operation, Output, invalid, notifications::Action, saved_state::notebook_key,
};
use crate::services::Result;
use lapis_app_protocol::database::state::{MetadataSnapshot, NotebookState, NotificationRecord};

impl Database {
    pub async fn get_notebook_state(&self, path: &str) -> Result<Option<NotebookState>> {
        self.get_meta(&notebook_key(path))
            .await?
            .map(serde_json::from_value)
            .transpose()
            .map_err(|_| invalid("Invalid stored notebook state"))
    }
    pub async fn set_notebook_state(&self, path: &str, state: NotebookState) -> Result<()> {
        let finite = state.mtime.is_finite()
            && state.updated_at.is_finite()
            && state.cells.values().all(|cell| {
                cell.duration_ms.is_none_or(f64::is_finite)
                    && cell.setup_phases.as_ref().is_none_or(|phases| {
                        phases
                            .iter()
                            .all(|phase| phase.duration_ms.is_none_or(f64::is_finite))
                    })
            });
        if !finite {
            return Err(invalid("Notebook state contains non-finite numbers"));
        }
        self.submit(Operation::SaveNotebook(path.into(), Some(state)))
            .await
            .map(|_| ())
    }
    pub async fn delete_notebook_state(&self, path: &str) -> Result<()> {
        self.submit(Operation::SaveNotebook(path.into(), None))
            .await
            .map(|_| ())
    }
    pub async fn save_metadata_snapshot(&self, snapshot: MetadataSnapshot) -> Result<()> {
        let finite = snapshot
            .file_cache
            .values()
            .all(|entry| entry.mtime.is_finite() && entry.size.is_finite())
            && snapshot
                .resolved_links
                .values()
                .chain(snapshot.unresolved_links.values())
                .all(|links| links.values().all(|count| count.is_finite()));
        if !finite {
            return Err(invalid("Metadata snapshot contains non-finite numbers"));
        }
        self.submit(Operation::SaveSnapshot(snapshot))
            .await
            .map(|_| ())
    }
    pub async fn load_metadata_snapshot(&self) -> Result<Option<MetadataSnapshot>> {
        match self.submit(Operation::LoadSnapshot).await? {
            Output::Snapshot(snapshot) => Ok(snapshot),
            _ => unreachable!("worker output matches operation"),
        }
    }
    pub async fn list_notifications(&self) -> Result<Vec<NotificationRecord>> {
        match self.submit(Operation::Notifications(Action::List)).await? {
            Output::Notifications(records) => Ok(records),
            _ => unreachable!("worker output matches operation"),
        }
    }
    pub async fn upsert_notification(&self, record: NotificationRecord) -> Result<()> {
        if !record.created_at.is_finite() || !record.updated_at.is_finite() {
            return Err(invalid("Notification contains non-finite timestamps"));
        }
        self.submit(Operation::Notifications(Action::Upsert(record)))
            .await
            .map(|_| ())
    }
    pub async fn mark_notification_read(&self, id: &str) -> Result<()> {
        self.submit(Operation::Notifications(Action::Read(id.into())))
            .await
            .map(|_| ())
    }
    pub async fn clear_notification(&self, id: &str) -> Result<()> {
        self.submit(Operation::Notifications(Action::Clear(id.into())))
            .await
            .map(|_| ())
    }
    pub async fn clear_all_notifications(&self) -> Result<()> {
        self.submit(Operation::Notifications(Action::ClearAll))
            .await
            .map(|_| ())
    }
}
