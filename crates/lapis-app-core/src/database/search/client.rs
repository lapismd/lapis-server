use super::{Action, Value, validation};
use crate::database::{Database, Operation, Output};
use crate::services::Result;
use lapis_app_protocol::database::search::{records::SearchDocumentRecord, *};

impl Database {
    pub async fn search_documents(
        &self,
        query: &str,
        options: retrieval::SearchOptions,
    ) -> Result<Vec<retrieval::SearchResult>> {
        validation::query_options(&options)?;
        match self
            .submit(Operation::Search(Action::Retrieve(
                query.into(),
                options,
                false,
            )))
            .await?
        {
            Output::Search(Value::Results(rows)) => Ok(rows),
            _ => unreachable!("matching worker output"),
        }
    }
    pub async fn search_document_paths(
        &self,
        query: &str,
        options: retrieval::SearchOptions,
    ) -> Result<Vec<String>> {
        validation::query_options(&options)?;
        match self
            .submit(Operation::Search(Action::Retrieve(
                query.into(),
                options,
                true,
            )))
            .await?
        {
            Output::Search(Value::Paths(paths)) => Ok(paths),
            _ => unreachable!("matching worker output"),
        }
    }

    /// Normalize domain records; the accepted owner completes configured inference
    /// and atomic publication even after the submitting client detaches.
    pub async fn upsert_search_document(&self, record: SearchDocumentRecord) -> Result<()> {
        let prepared = super::preparation::normalize(record)?;
        self.submit(Operation::Search(Action::PutSource(Box::new(prepared))))
            .await
            .map(|_| ())
    }
    /// Native processor boundary. Callers finish normalization and inference first.
    pub async fn upsert_prepared_search_document(
        &self,
        record: SearchDocumentRecord,
    ) -> Result<()> {
        validation::record(&record)?;
        self.submit(Operation::Search(Action::Put(Box::new(record))))
            .await
            .map(|_| ())
    }
    pub async fn delete_search_document(&self, path: &str) -> Result<()> {
        self.submit(Operation::Search(Action::Delete(path.into())))
            .await
            .map(|_| ())
    }
    pub async fn get_search_document(&self, path: &str) -> Result<Option<SearchDocumentRecord>> {
        match self
            .submit(Operation::Search(Action::Get(path.into())))
            .await?
        {
            Output::Search(Value::Document(row)) => Ok(row.map(|r| *r)),
            _ => unreachable!("matching worker output"),
        }
    }
    pub async fn list_search_documents(&self) -> Result<Vec<SearchDocumentRecord>> {
        match self.submit(Operation::Search(Action::List)).await? {
            Output::Search(Value::Documents(rows)) => Ok(rows),
            _ => unreachable!("matching worker output"),
        }
    }
    pub async fn list_search_document_manifest(
        &self,
        query: SearchManifestQuery,
    ) -> Result<SearchManifestPage> {
        crate::database::indexed::limit(query.limit, 500, 1)?;
        match self
            .submit(Operation::Search(Action::Manifest(query)))
            .await?
        {
            Output::Search(Value::Manifest(page)) => Ok(page),
            _ => unreachable!("matching worker output"),
        }
    }
    pub async fn get_search_index_stats(&self) -> Result<SearchIndexStats> {
        match self.submit(Operation::Search(Action::Stats)).await? {
            Output::Search(Value::Stats(stats)) => Ok(stats),
            _ => unreachable!("matching worker output"),
        }
    }
}

impl crate::database::Database {
    pub(crate) fn validate_search_chunking(
        chunking: &lapis_app_protocol::database::search::source::SearchChunkingSettings,
    ) -> crate::services::Result<()> {
        super::preparation::validate_chunking(chunking)
    }
}
