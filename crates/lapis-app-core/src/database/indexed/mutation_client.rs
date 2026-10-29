use super::{Action, client::validate_record};
use crate::database::{Database, Operation, invalid, tasks};
use crate::services::Result;
use lapis_app_protocol::database::indexed::{mutations::IndexedFile, records::CoreIndexedFile};

impl Database {
    pub async fn upsert_indexed_file(&self, input: IndexedFile) -> Result<()> {
        let IndexedFile {
            file,
            metadata,
            links,
            tags,
            properties,
            task,
        } = input;
        let core = CoreIndexedFile {
            file,
            metadata,
            links,
            tags,
            properties,
        };
        validate_record(&core)?;
        if task
            .as_ref()
            .and_then(Option::as_ref)
            .is_some_and(|row| !tasks::valid(row))
        {
            return Err(invalid("Invalid indexed task numeric data"));
        }
        self.submit(Operation::Indexed(Action::Upsert(
            Box::new(core),
            task.map(|v| v.map(Box::new)),
        )))
        .await
        .map(|_| ())
    }
    pub async fn delete_indexed_file(&self, path: &str) -> Result<()> {
        self.submit(Operation::Indexed(Action::Delete(path.into())))
            .await
            .map(|_| ())
    }
    pub async fn rename_indexed_file(&self, old: &str, new: &str) -> Result<()> {
        self.submit(Operation::Indexed(Action::Rename(old.into(), new.into())))
            .await
            .map(|_| ())
    }
}
