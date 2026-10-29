mod client;
mod deletion;
mod facets;
mod filter_sql;
mod mutation_client;
mod normalize;
mod reads;
mod rename;
mod write_domains;
mod writes;

use super::{MAX_BYTES, engine, invalid, store::Store};
use crate::services::Result;
use lapis_app_protocol::database::indexed::{queries::*, records::*};
use serde::Serialize;

#[derive(Serialize)]
pub(super) enum Action {
    Put(Box<CoreIndexedFile>),
    Upsert(
        Box<CoreIndexedFile>,
        Option<Option<Box<lapis_app_protocol::database::tasks::record::TaskRecord>>>,
    ),
    Delete(String),
    Rename(String, String),
    Get(String),
    Manifest(ManifestQuery),
    Page(MetadataPageQuery),
    Facets(FacetQuery),
    Links(LinkQuery),
}
pub(super) enum Value {
    Done,
    File(Option<Box<IndexedRow>>),
    Manifest(ManifestPage),
    Page(MetadataPage),
    Facets(Vec<FacetRow>),
    Links(Vec<LinkRecord>),
}
impl Store {
    pub async fn indexed(&mut self, action: Action) -> Result<Value> {
        match action {
            Action::Upsert(record, task) => {
                let path = record.file.path.clone();
                self.put_indexed(*record).await?;
                if let Some(task) = task {
                    let action = if let Some(task) = task {
                        crate::database::tasks::Action::Put(task)
                    } else {
                        crate::database::tasks::Action::Delete(path)
                    };
                    self.tasks(action).await?;
                }
                Ok(Value::Done)
            }
            Action::Delete(path) => self.delete_indexed(&path).await.map(|_| Value::Done),
            Action::Rename(old, new) => self.rename_indexed(&old, &new).await.map(|_| Value::Done),
            Action::Put(record) => self.put_indexed(*record).await.map(|_| Value::Done),
            Action::Get(path) => self
                .read_indexed(&[path], None, false)
                .await
                .map(|mut rows| Value::File(rows.pop().map(Box::new))),
            Action::Manifest(query) => self.indexed_manifest(query).await.map(Value::Manifest),
            Action::Page(query) => self.indexed_page(query).await.map(Value::Page),
            Action::Facets(query) => self.indexed_facets(query).await.map(Value::Facets),
            Action::Links(query) => self.indexed_links(query).await.map(Value::Links),
        }
    }
}
pub(super) struct Budget(usize);
impl Budget {
    fn new() -> Self {
        Self(0)
    }
    fn take(&mut self, bytes: usize) -> Result<()> {
        self.0 = self.0.saturating_add(bytes);
        if self.0 >= MAX_BYTES {
            Err(invalid("Indexed result exceeds 32 MiB"))
        } else {
            Ok(())
        }
    }
}
pub(super) fn slots(count: usize) -> String {
    vec!["?"; count].join(",")
}
pub(super) fn limit(value: Option<usize>, default: usize, minimum: usize) -> Result<usize> {
    let n = value.unwrap_or(default).max(minimum);
    if n >= i64::MAX as usize {
        return Err(invalid("Indexed query limit is too large"));
    }
    Ok(n)
}
pub(super) fn text(value: impl Into<String>) -> turso::Value {
    turso::Value::Text(value.into())
}
pub(super) fn number(row: &turso::Row, index: usize) -> Result<f64> {
    match row.get_value(index).map_err(engine)? {
        turso::Value::Integer(v) => Ok(v as f64),
        turso::Value::Real(v) if v.is_finite() => Ok(v),
        _ => Err(invalid("Invalid indexed number")),
    }
}
pub(super) fn optional_text(row: &turso::Row, index: usize) -> Result<Option<String>> {
    match row.get_value(index).map_err(engine)? {
        turso::Value::Null => Ok(None),
        turso::Value::Text(s) => Ok(Some(s)),
        _ => Err(invalid("Invalid indexed text")),
    }
}
