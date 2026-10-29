mod enhancement;
mod evaluation;
mod lexer;
mod page;
mod parser;
mod properties;
mod query;
mod scopes;
mod semantic;
mod snippets;
mod text;
use super::{Value, text as sql_text};
use crate::database::{MAX_BYTES, engine, invalid, store::Store};
use crate::services::Result;
use lapis_app_protocol::database::{
    indexed::records::PropertyRecord,
    search::{records::SearchDocumentRecord, retrieval::*},
};

impl Store {
    pub(super) async fn retrieve_search(
        &self,
        raw: String,
        options: SearchOptions,
        paths: bool,
    ) -> Result<Value> {
        let query = query::Query::new(raw)?;
        if self.embedding.config.is_some() {
            return self.semantic_search(&query, &options, paths).await;
        }
        if !matches!(options.mode, Some(SearchMode::Lexical))
            && self
                .get("search.embeddingProvider")
                .await?
                .is_some_and(|value| !value.is_null())
        {
            return Err(crate::services::Error::new(
                lapis_app_protocol::ErrorCode::Unsupported,
                "Native semantic Search is not configured",
            ));
        }
        let mut page = page::Page::new(options.limit.unwrap_or(100), paths)?;
        let mut clauses = vec![];
        let mut args = vec![];
        if let Some(ids) = options
            .source_provider_ids
            .as_ref()
            .filter(|v| !v.is_empty())
        {
            clauses.push(format!(
                "source_provider_id IN ({})",
                vec!["?"; ids.len()].join(",")
            ));
            args.extend(ids.iter().map(sql_text));
        }
        if let Some(prefix) = options.path_prefix.as_ref().filter(|s| !s.is_empty()) {
            clauses.push("(path=? OR instr(path,?)=1)".into());
            args.extend([sql_text(prefix), sql_text(format!("{prefix}/"))]);
        }
        let table =
            options.case_sensitive.unwrap_or(false) || query.structured() || !clauses.is_empty();
        if !table && matches!(options.mode, Some(SearchMode::Vector)) {
            return page.finish(&query, &options, 0);
        }
        let terms = query.collected().unwrap_or_default();
        let (sql, args) = if !table && !terms.is_empty() {
            let fts = terms
                .iter()
                .map(|s| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")))
                .collect::<Vec<_>>()
                .join(" AND ");
            ("SELECT data_json,fts_score(name,path,content,tags,metadata_text,?) AS score FROM search_docs WHERE fts_match(name,path,content,tags,metadata_text,?) ORDER BY score ASC LIMIT ?".to_owned(),vec![sql_text(fts.clone()),sql_text(fts),turso::Value::Integer(options.limit.unwrap_or(100) as i64)])
        } else {
            (
                format!(
                    "SELECT data_json FROM search_docs {} ORDER BY path",
                    if clauses.is_empty() {
                        String::new()
                    } else {
                        format!("WHERE {}", clauses.join(" AND "))
                    }
                ),
                args,
            )
        };
        let required = query.properties();
        let mut candidates = 0usize;
        let mut rows = self.connection.query(&sql, args).await.map_err(engine)?;
        while let Some(row) = rows.next().await.map_err(engine)? {
            let raw = row.get::<String>(0).map_err(engine)?;
            if raw.len() >= MAX_BYTES {
                return Err(invalid("Stored Search record exceeds bounds"));
            }
            let record: SearchDocumentRecord =
                serde_json::from_str(&raw).map_err(|_| invalid("Invalid stored Search record"))?;
            let props = properties::Properties::new(
                self.search_properties(&record.path).await?,
                record.metadata_text.as_deref(),
            );
            if !props.has_required(&required) {
                continue;
            }
            candidates = candidates.saturating_add(1);
            let score = evaluation::score(
                &query,
                &record,
                &props,
                options.case_sensitive.unwrap_or(false),
                &page.locale,
            );
            page.push(record, score)?;
        }
        page.finish(&query, &options, candidates)
    }
    async fn search_properties(&self, path: &str) -> Result<Vec<PropertyRecord>> {
        let mut rows=self.connection.query("SELECT p.data_json FROM metadata_properties p JOIN files f ON f.path=p.path WHERE p.path=? AND f.indexed=1 AND f.deleted=0 ORDER BY p.ordinal",[path]).await.map_err(engine)?;
        let mut result = vec![];
        let mut bytes = 0;
        while let Some(row) = rows.next().await.map_err(engine)? {
            let raw = row.get::<String>(0).map_err(engine)?;
            bytes += raw.len();
            if bytes >= MAX_BYTES {
                return Err(invalid("Search properties exceed bounds"));
            }
            result
                .push(serde_json::from_str(&raw).map_err(|_| invalid("Invalid Search property"))?);
        }
        Ok(result)
    }
}
