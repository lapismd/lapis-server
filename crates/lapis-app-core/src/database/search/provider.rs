use super::{embedding, text};
use crate::{
    database::{Database, Operation, Output, closed, engine, store::Store},
    services::Result,
};
use lapis_app_protocol::database::{
    changes::ChangeDomain,
    search::embedding::{SearchEmbeddingProviderConfig as Config, SearchEmbeddingRuntimeStatus},
};

impl Database {
    pub async fn configure_search_embedding_provider(&self, config: Option<Config>) -> Result<()> {
        embedding::validate(&config)?;
        self.submit(Operation::Search(super::Action::Configure(config)))
            .await
            .map(|_| ())
    }
    pub async fn get_search_embedding_provider(&self) -> Result<Option<Config>> {
        match self
            .submit(Operation::Search(super::Action::ProviderConfig))
            .await?
        {
            Output::Search(super::Value::ProviderConfig(config)) => Ok(config),
            _ => unreachable!("matching worker output"),
        }
    }
    /// Observe the current native worker without queuing behind model inference.
    pub fn get_search_embedding_runtime_status(
        &self,
    ) -> Result<Option<SearchEmbeddingRuntimeStatus>> {
        if self.admission.lock().map_err(|_| closed())?.is_none() {
            return Err(closed());
        }
        Ok(self.embedding_status.borrow().clone())
    }
}
impl Store {
    pub(in crate::database) async fn configure_search_provider(
        &mut self,
        config: Option<Config>,
    ) -> Result<()> {
        embedding::validate(&config)?;
        if config == self.embedding.config {
            return Ok(());
        }
        self.begin().await?;
        let result = async {
            if let Some(config) = &config {
                let mut after: Option<String> = None;
                loop {
                    let records = match &after { Some(path) => self.search_rows("SELECT data_json FROM search_docs WHERE path>? ORDER BY path LIMIT 100",vec![text(path)]).await?, None => self.search_rows("SELECT data_json FROM search_docs ORDER BY path LIMIT 100",vec![]).await? };
                    if records.is_empty() { break; }
                    for mut record in records {
                        after = Some(record.path.clone());
                        self.embedding.pending(&mut record,config);
                        self.write_search(&record).await?;
                    }
                }
            }
            self.connection.execute("INSERT INTO app_meta(key,value_json) VALUES('search.embeddingProvider',?) ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json",[super::json(&config)?]).await.map_err(engine)?;
            Ok(((),Some(self.journal(ChangeDomain::Search,vec![]).await?)))
        }.await;
        self.finish(result).await?;
        self.embedding.reset(config);
        Ok(())
    }
}
