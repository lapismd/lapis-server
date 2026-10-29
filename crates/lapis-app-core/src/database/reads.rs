use super::{MAX_BYTES, MetaEntry, MetaPage, MetaScan, encoded, engine, invalid, store::Store};
use crate::services::Result;
use serde_json::Value;

impl Store {
    pub async fn get(&self, key: &str) -> Result<Option<Value>> {
        let mut rows = self
            .connection
            .query("SELECT value_json FROM app_meta WHERE key=?", [key])
            .await
            .map_err(engine)?;
        let Some(row) = rows.next().await.map_err(engine)? else {
            return Ok(None);
        };
        parse(row.get::<String>(0).map_err(engine)?).map(Some)
    }
    pub async fn scan(&self, query: MetaScan) -> Result<MetaPage> {
        let limit = query.limit.unwrap_or(100).clamp(1, 500);
        let after = query.after.as_deref().unwrap_or(&query.prefix);
        let mut rows = self.connection.query(
            "SELECT key,value_json FROM app_meta WHERE substr(key,1,?)=? AND key>? ORDER BY key ASC LIMIT ?",
            turso::params![query.prefix.chars().count() as i64,query.prefix.as_str(),after,(limit+1) as i64]
        ).await.map_err(engine)?;
        let mut entries: Vec<MetaEntry> = Vec::new();
        let mut bytes = 0;
        let mut next_cursor = None;
        while let Some(row) = rows.next().await.map_err(engine)? {
            if entries.len() == limit {
                next_cursor = entries.last().map(|entry| entry.key.clone());
                break;
            }
            let entry = MetaEntry {
                key: row.get::<String>(0).map_err(engine)?,
                value: parse(row.get::<String>(1).map_err(engine)?)?,
            };
            bytes += encoded(&entry)?.len();
            if bytes >= MAX_BYTES {
                return Err(invalid("Application database result exceeds 32 MiB"));
            }
            entries.push(entry);
        }
        let page = MetaPage {
            entries,
            next_cursor,
        };
        encoded(&page)?;
        Ok(page)
    }
}
pub(super) fn parse(text: String) -> Result<Value> {
    if text.len() >= MAX_BYTES {
        return Err(invalid("Application metadata value exceeds 32 MiB"));
    }
    serde_json::from_str(&text).map_err(|_| invalid("Application metadata contains invalid JSON"))
}
