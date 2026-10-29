use super::{Database, Operation, Output, invalid};
use crate::services::Result;
use lapis_app_protocol::database::history::{FileHistory, StoreHistory, StoredHistory};

impl Database {
    pub async fn get_file_history(&self, path: &str) -> Result<Option<FileHistory>> {
        match self.submit(Operation::GetHistory(path.into())).await? {
            Output::History(history) => Ok(history),
            _ => unreachable!("worker output matches operation"),
        }
    }
    pub async fn store_file_history_revision(&self, input: StoreHistory) -> Result<StoredHistory> {
        if !input.created_at.is_finite()
            || ![input.source_mtime, input.source_size, input.max_revisions]
                .into_iter()
                .all(|n| n.is_none_or(f64::is_finite))
        {
            return Err(invalid("File history contains non-finite numbers"));
        }
        match self.submit(Operation::StoreHistory(input)).await? {
            Output::StoredHistory(history) => Ok(history),
            _ => unreachable!("worker output matches operation"),
        }
    }
}
