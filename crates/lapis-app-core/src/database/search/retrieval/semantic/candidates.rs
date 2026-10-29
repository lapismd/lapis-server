use super::super::query::Query;
use crate::database::{MAX_BYTES, engine, invalid, store::Store};
use crate::services::Result;
use lapis_app_protocol::database::search::retrieval::{SearchMode, SearchOptions};
use std::collections::HashSet;

pub(super) struct Candidates {
    sql: String,
    args: Vec<turso::Value>,
    paths: Option<Vec<String>>,
}
fn include(paths: &mut HashSet<String>, bytes: &mut usize, path: String) -> Result<()> {
    if paths.contains(&path) {
        return Ok(());
    }
    *bytes += path.len() + 64;
    if *bytes >= MAX_BYTES {
        return Err(invalid("Search candidate paths exceed bounds"));
    }
    paths.insert(path);
    Ok(())
}
impl Candidates {
    pub fn batch_count(&self) -> usize {
        self.paths
            .as_ref()
            .map_or(1, |paths| paths.len().div_ceil(256))
    }
    pub fn batch(&self, index: usize) -> (String, Vec<turso::Value>) {
        if let Some(paths) = &self.paths {
            let start = index * 256;
            let paths = &paths[start..paths.len().min(start + 256)];
            (
                format!(
                    "SELECT path,data_json FROM search_docs WHERE path IN ({}) ORDER BY path",
                    vec!["?"; paths.len()].join(",")
                ),
                paths.iter().cloned().map(turso::Value::Text).collect(),
            )
        } else {
            (self.sql.clone(), self.args.clone())
        }
    }
    pub async fn new(
        store: &Store,
        query: &Query,
        options: &SearchOptions,
        vector: Option<&[f64]>,
    ) -> Result<Self> {
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
            args.extend(ids.iter().map(|s| turso::Value::Text(s.clone())));
        }
        if let Some(prefix) = options.path_prefix.as_ref().filter(|s| !s.is_empty()) {
            clauses.push("(path=? OR instr(path,?)=1)".into());
            args.extend([
                turso::Value::Text(prefix.clone()),
                turso::Value::Text(format!("{prefix}/")),
            ]);
        }
        let table =
            options.case_sensitive.unwrap_or(false) || query.structured() || !clauses.is_empty();
        let paths = if table {
            None
        } else {
            let mut paths = HashSet::new();
            let mut bytes = 0;
            let limit = options.limit.unwrap_or(100) as i64;
            let text = |s: String| turso::Value::Text(s);
            if !matches!(options.mode, Some(SearchMode::Vector)) {
                let terms = query.collected().unwrap_or_default();
                let (sql, args) = if terms.is_empty() {
                    ("SELECT path FROM search_docs ORDER BY path", vec![])
                } else {
                    let fts = terms
                        .iter()
                        .map(|s| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")))
                        .collect::<Vec<_>>()
                        .join(" AND ");
                    (
                        "SELECT path FROM search_docs WHERE fts_match(name,path,content,tags,metadata_text,?) ORDER BY fts_score(name,path,content,tags,metadata_text,?) ASC LIMIT ?",
                        vec![text(fts.clone()), text(fts), turso::Value::Integer(limit)],
                    )
                };
                let mut rows = store.connection.query(sql, args).await.map_err(engine)?;
                while let Some(row) = rows.next().await.map_err(engine)? {
                    include(
                        &mut paths,
                        &mut bytes,
                        row.get::<String>(0).map_err(engine)?,
                    )?;
                }
            }
            if let Some(vector) = vector {
                let mut rows = store.connection.query("SELECT path FROM search_chunks WHERE embedding IS NOT NULL ORDER BY vector_distance_cos(embedding,vector32(?)) ASC LIMIT ?",vec![text(serde_json::to_string(vector).map_err(|_|invalid("Invalid Search vector"))?),turso::Value::Integer(limit)]).await.map_err(engine)?;
                while let Some(row) = rows.next().await.map_err(engine)? {
                    include(
                        &mut paths,
                        &mut bytes,
                        row.get::<String>(0).map_err(engine)?,
                    )?;
                }
            }
            let mut paths = paths.into_iter().collect::<Vec<_>>();
            paths.sort();
            Some(paths)
        };
        let scope = if clauses.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", clauses.join(" AND "))
        };
        Ok(Self {
            sql: format!("SELECT path,data_json FROM search_docs {scope} ORDER BY path"),
            args,
            paths,
        })
    }
}
