mod candidates;
mod ranking;
mod result;
use super::{evaluation, properties, query::Query};
use crate::database::{MAX_BYTES, engine, invalid, store::Store};
use crate::services::Result;
use icu_collator::CollatorBorrowed;
use lapis_app_protocol::database::search::{records::SearchDocumentRecord, retrieval::*};

impl Store {
    pub(super) async fn semantic_search(
        &self,
        query: &Query,
        options: &SearchOptions,
        paths: bool,
    ) -> Result<super::super::Value> {
        let vector = if matches!(options.mode, Some(SearchMode::Lexical)) {
            None
        } else {
            self.embedding.query_vector(&query.raw).await
        };
        let candidates =
            candidates::Candidates::new(self, query, options, vector.as_deref()).await?;
        let locale = CollatorBorrowed::try_new(Default::default(), Default::default())
            .map_err(|_| invalid("Search collation is unavailable"))?;
        let required = query.properties();
        let mut entries = vec![];
        let mut bytes = 0;
        for batch in 0..candidates.batch_count() {
            let (sql, args) = candidates.batch(batch);
            let mut rows = self.connection.query(&sql, args).await.map_err(engine)?;
            while let Some(row) = rows.next().await.map_err(engine)? {
                let raw = row.get::<String>(1).map_err(engine)?;
                if raw.len() >= MAX_BYTES {
                    return Err(invalid("Stored Search record exceeds bounds"));
                }
                let record: SearchDocumentRecord = serde_json::from_str(&raw)
                    .map_err(|_| invalid("Invalid stored Search record"))?;
                let props = properties::Properties::new(
                    self.search_properties(&record.path).await?,
                    record.metadata_text.as_deref(),
                );
                if !props.has_required(&required) {
                    continue;
                }
                let lexical = evaluation::score(
                    query,
                    &record,
                    &props,
                    options.case_sensitive.unwrap_or(false),
                    &locale,
                );
                let entry = ranking::Entry::new(&record, lexical, vector.as_deref());
                ranking::admit(&mut bytes, &entry)?;
                entries.push(entry);
            }
        }
        let lexical_candidates = entries.len();
        let vector_candidates = ranking::rank(&mut entries, &locale);
        let mode = if vector.is_some() {
            if matches!(options.mode, Some(SearchMode::Vector)) {
                SearchRetrievalMode::Vector
            } else if vector_candidates > 0 {
                SearchRetrievalMode::Hybrid
            } else {
                SearchRetrievalMode::Lexical
            }
        } else {
            SearchRetrievalMode::Lexical
        };
        entries.retain(|e| match mode {
            SearchRetrievalMode::Vector => e.vector >= ranking::MIN_VECTOR_SCORE,
            SearchRetrievalMode::Hybrid => e.lexical > 0. || e.vector >= ranking::MIN_VECTOR_SCORE,
            SearchRetrievalMode::Lexical => e.lexical > 0.,
        });
        entries.sort_by(|a, b| ranking::compare(a, b, mode, &locale));
        let enhancement = super::enhancement::resolve(
            options,
            lapis_app_protocol::database::search::enhancement::SearchEnhancementSignal {
                requested_mode: options.mode.unwrap_or_default(),
                top_lexical_score: entries.iter().map(|e| e.lexical).fold(0., f64::max),
                top_vector_score: entries.iter().map(|e| e.vector).fold(0., f64::max),
                top_fused_score: entries.first().map(|e| e.score(mode).fused).unwrap_or(0.),
                result_count: entries.len(),
            },
            mode,
        );
        entries.truncate(options.limit.unwrap_or(100));
        if paths {
            let paths = entries.into_iter().map(|e| e.path).collect::<Vec<_>>();
            crate::database::encoded(&paths)?;
            return Ok(super::super::Value::Paths(paths));
        }
        let mut output = vec![];
        let mut bytes = 0;
        for entry in entries {
            let mut rows = self
                .search_rows(
                    "SELECT data_json FROM search_docs WHERE path=?",
                    vec![turso::Value::Text(entry.path.clone())],
                )
                .await?;
            let record = rows
                .pop()
                .ok_or_else(|| invalid("Selected Search record is unavailable"))?;
            let result = result::build(
                record,
                entry,
                query,
                options,
                result::Context {
                    enhancement: enhancement.as_ref(),
                    mode,
                    lexical_candidates,
                    vector_candidates,
                    config: self.embedding.config.as_ref(),
                },
            );
            bytes += crate::database::encoded(&result)?.len();
            if bytes >= MAX_BYTES {
                return Err(invalid("Search selected records exceed bounds"));
            }
            output.push(result);
        }
        crate::database::encoded(&output)?;
        Ok(super::super::Value::Results(output))
    }
}
