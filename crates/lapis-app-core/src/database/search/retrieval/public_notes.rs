use super::{SearchMode, SearchOptions, SearchRetrievalMode, Value, query::Query};
use crate::{
    database::{engine, store::Store},
    services::{Error, Result},
};
use lapis_app_protocol::{
    ErrorCode,
    content::validate_content_path,
    database::search::embedding::{SearchEmbeddingProviderKind, SearchEmbeddingRuntimePhase},
};

const MARKDOWN: &str = "source_provider_id='search:markdown' AND lower(extension) IN ('md','markdown') AND substr(path,1,1)<>'.' AND instr(path,'/.')=0";

pub(super) fn public_markdown(path: &str) -> bool {
    validate_content_path(path).is_ok() && !path.split('/').any(|part| part.starts_with('.'))
}

pub(super) fn scope(options: &SearchOptions) -> (String, Vec<turso::Value>) {
    match &options.path_prefix {
        Some(folder) => (
            format!("{MARKDOWN} AND instr(path,?)=1"),
            vec![turso::Value::Text(format!("{folder}/"))],
        ),
        None => (MARKDOWN.into(), Vec::new()),
    }
}

impl Store {
    pub(in crate::database::search) async fn retrieve_notes(
        &self,
        raw: String,
        mut options: SearchOptions,
    ) -> Result<super::super::NoteSearch> {
        let requested = options.mode.unwrap_or_default();
        let semantic_status = self.note_embeddings(&options).await?;
        let mut mode = match (requested, semantic_status) {
            (SearchMode::Lexical, _) => SearchMode::Lexical,
            (SearchMode::Auto, "ready") => SearchMode::Hybrid,
            (SearchMode::Auto, _) => SearchMode::Lexical,
            (mode, "ready") => mode,
            (_, "disabled") => {
                return Err(Error::new(
                    ErrorCode::Unsupported,
                    "Enable a local semantic search model in this vault's Search settings",
                ));
            }
            (_, "indexing") => {
                return Err(Error::new(
                    ErrorCode::Busy,
                    "Semantic search is indexing; retry when the vault's embeddings are ready",
                ));
            }
            _ => {
                return Err(Error::new(
                    ErrorCode::Io,
                    "The local semantic search model is unavailable",
                ));
            }
        };
        let query = Query::plain(raw);
        options.mode = Some(mode);
        let result = self.semantic_search(&query, &options, false, true).await;
        let (result, semantic_status) = match result {
            Err(error)
                if matches!(requested, SearchMode::Auto) && error.code() == ErrorCode::Io =>
            {
                mode = SearchMode::Lexical;
                options.mode = Some(mode);
                (
                    self.semantic_search(&query, &options, false, true).await?,
                    "unavailable",
                )
            }
            other => (other?, semantic_status),
        };
        let Value::Results(rows) = result else {
            unreachable!("note search returns rows")
        };
        let mode = match mode {
            SearchMode::Vector => SearchRetrievalMode::Vector,
            SearchMode::Hybrid => SearchRetrievalMode::Hybrid,
            SearchMode::Auto | SearchMode::Lexical => SearchRetrievalMode::Lexical,
        };
        Ok(super::super::NoteSearch {
            rows,
            mode,
            semantic_status,
        })
    }

    async fn note_embeddings(&self, options: &SearchOptions) -> Result<&'static str> {
        if !self
            .embedding
            .config
            .as_ref()
            .is_some_and(|config| config.kind == SearchEmbeddingProviderKind::TransformersJs)
        {
            return Ok("disabled");
        }
        let failed = self
            .embedding
            .status
            .borrow()
            .as_ref()
            .is_some_and(|state| matches!(state.phase, SearchEmbeddingRuntimePhase::Error));
        let (filter, args) = scope(options);
        let mut rows = self.connection.query(
            &format!("SELECT 1 FROM search_chunks WHERE embedding IS NULL AND path IN (SELECT path FROM search_docs WHERE {filter}) LIMIT 1"),
            args,
        ).await.map_err(engine)?;
        Ok(
            match (rows.next().await.map_err(engine)?.is_some(), failed) {
                (true, true) => "unavailable",
                (true, false) => "indexing",
                (false, _) => "ready",
            },
        )
    }
}
