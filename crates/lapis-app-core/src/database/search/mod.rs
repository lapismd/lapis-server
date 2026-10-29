mod client;
mod controls;
pub(in crate::database) mod embedding;
mod preparation;
mod provider;
mod reads;
mod retrieval;
mod validation;
mod writes;
use crate::database::{encoded, invalid, store::Store};
use crate::services::Result;
use lapis_app_protocol::database::search::{records::SearchDocumentRecord, *};
use serde::Serialize;

#[derive(Serialize)]
pub(super) enum Action {
    Barrier,
    Rebuild,
    Configure(Option<embedding_config::SearchEmbeddingProviderConfig>),
    ProviderConfig,
    PutSource(Box<SearchDocumentRecord>),
    Put(Box<SearchDocumentRecord>),
    Delete(String),
    Get(String),
    List,
    Manifest(SearchManifestQuery),
    Stats,
    Retrieve(
        String,
        lapis_app_protocol::database::search::retrieval::SearchOptions,
        bool,
    ),
}
pub(super) enum Value {
    ProviderConfig(Option<embedding_config::SearchEmbeddingProviderConfig>),
    Done,
    Document(Option<Box<SearchDocumentRecord>>),
    Documents(Vec<SearchDocumentRecord>),
    Manifest(SearchManifestPage),
    Stats(SearchIndexStats),
    Results(Vec<lapis_app_protocol::database::search::retrieval::SearchResult>),
    Paths(Vec<String>),
}
impl Store {
    pub(super) async fn search(&mut self, action: Action) -> Result<Value> {
        match action {
            Action::Barrier => Ok(Value::Done),
            Action::Rebuild => self.rebuild_search().await.map(|_| Value::Done),
            Action::Configure(config) => self
                .configure_search_provider(config)
                .await
                .map(|_| Value::Done),
            Action::ProviderConfig => Ok(Value::ProviderConfig(self.embedding.config.clone())),
            Action::PutSource(record) => {
                let record = self.embedding.prepare(*record).await;
                self.mutate_search(Action::Put(Box::new(record))).await
            }
            Action::Retrieve(query, options, paths) => {
                self.retrieve_search(query, options, paths).await
            }
            Action::Get(path) => self
                .search_rows(
                    "SELECT data_json FROM search_docs WHERE path=?",
                    vec![text(path)],
                )
                .await
                .map(|mut rows| Value::Document(rows.pop().map(Box::new))),
            Action::List => self
                .search_rows("SELECT data_json FROM search_docs ORDER BY path", vec![])
                .await
                .map(Value::Documents),
            Action::Manifest(query) => self.search_manifest(query).await.map(Value::Manifest),
            Action::Stats => self.search_stats().await.map(Value::Stats),
            write => self.mutate_search(write).await,
        }
    }
}
use lapis_app_protocol::database::search::embedding as embedding_config;
fn text(value: impl Into<String>) -> turso::Value {
    turso::Value::Text(value.into())
}
fn optional_text(value: Option<&str>) -> turso::Value {
    value.map_or(turso::Value::Null, text)
}
fn optional_number(value: Option<f64>) -> turso::Value {
    value.map_or(turso::Value::Null, turso::Value::Real)
}
fn json(value: &impl Serialize) -> Result<String> {
    String::from_utf8(encoded(value)?).map_err(|_| invalid("Invalid Search JSON"))
}
