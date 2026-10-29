use super::{Action, preparation, text};
use crate::database::{Database, MAX_BYTES, Operation, encoded, invalid, store::Store};
use crate::services::Result;
use lapis_app_protocol::database::{changes::ChangeDomain, search::records::SearchDocumentRecord};

impl Database {
    /// Retain the provider's no-op batch contract as an admitted owner barrier.
    pub async fn begin_search_indexing_batch(&self) -> Result<()> {
        self.submit(Operation::Search(Action::Barrier))
            .await
            .map(|_| ())
    }
    pub async fn end_search_indexing_batch(&self) -> Result<()> {
        self.submit(Operation::Search(Action::Barrier))
            .await
            .map(|_| ())
    }
    /// Re-prepare stored source records; accepted pages outlive the waiter.
    pub async fn rebuild_search_index(&self) -> Result<()> {
        self.submit(Operation::Search(Action::Rebuild))
            .await
            .map(|_| ())
    }
}

impl Store {
    pub(super) async fn rebuild_search(&mut self) -> Result<()> {
        let mut after: Option<String> = None;
        let mut rebuilt = false;
        loop {
            let records = match &after {
                Some(path) => {
                    self.search_rows(
                        "SELECT data_json FROM search_docs WHERE path>? ORDER BY path LIMIT 100",
                        vec![text(path)],
                    )
                    .await?
                }
                None => {
                    self.search_rows(
                        "SELECT data_json FROM search_docs ORDER BY path LIMIT 100",
                        vec![],
                    )
                    .await?
                }
            };
            if records.is_empty() {
                if !rebuilt {
                    self.publish_rebuilt_page(&[]).await?;
                }
                return Ok(());
            }
            after = records.last().map(|record| record.path.clone());
            let mut prepared = Vec::with_capacity(records.len());
            let mut bytes = 0usize;
            for record in records {
                let record = self
                    .embedding
                    .prepare(preparation::normalize(record)?)
                    .await;
                bytes = bytes.saturating_add(encoded(&record)?.len() + 1);
                if bytes >= MAX_BYTES {
                    return Err(invalid("Rebuilt Search page exceeds bounds"));
                }
                prepared.push(record);
            }
            self.publish_rebuilt_page(&prepared).await?;
            rebuilt = true;
        }
    }
    async fn publish_rebuilt_page(&mut self, records: &[SearchDocumentRecord]) -> Result<()> {
        self.begin().await?;
        let result = async {
            for record in records {
                self.write_search(record).await?;
            }
            let paths = records.iter().map(|record| record.path.clone()).collect();
            Ok(((), Some(self.journal(ChangeDomain::Search, paths).await?)))
        }
        .await;
        self.finish(result).await
    }
}
