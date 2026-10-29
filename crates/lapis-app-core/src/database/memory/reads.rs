use super::{filter, text};
use crate::database::{MAX_BYTES, encoded, engine, invalid, store::Store};
use crate::services::Result;
use lapis_app_protocol::database::memory::records::*;
use serde::{Serialize, de::DeserializeOwned};

impl Store {
    pub(super) async fn memory_rows<T: DeserializeOwned + Serialize>(
        &self,
        sql: &str,
        args: Vec<turso::Value>,
    ) -> Result<Vec<T>> {
        let mut rows = self.connection.query(sql, args).await.map_err(engine)?;
        let mut result = vec![];
        let mut bytes = 0usize;
        while let Some(row) = rows.next().await.map_err(engine)? {
            let json = row.get::<String>(0).map_err(engine)?;
            bytes = bytes.saturating_add(json.len());
            if bytes >= MAX_BYTES || result.len() >= 100_000 {
                return Err(invalid("Memory results exceed bounds"));
            }
            result.push(
                serde_json::from_str(&json).map_err(|_| invalid("Invalid stored memory record"))?,
            );
        }
        encoded(&result)?;
        Ok(result)
    }
    pub(super) async fn memory_source(&self, key: &str) -> Result<Option<MemorySource>> {
        self.memory_rows(
            "SELECT data_json FROM memory_source_state WHERE source_key=?",
            vec![text(key)],
        )
        .await
        .map(|mut r| r.pop())
    }
    pub(super) async fn memory_signals(
        &self,
        target: Option<&str>,
    ) -> Result<Vec<MemoryRecallSignal>> {
        let (filter, args) = filter("target_ref", target);
        let mut rows=self.connection.query(&format!("SELECT data_json,best_score,hit_count FROM memory_recall_signals{filter} ORDER BY day DESC,query_fingerprint"),args).await.map_err(engine)?;
        let mut result = vec![];
        let mut bytes = 0usize;
        while let Some(row) = rows.next().await.map_err(engine)? {
            let json = row.get::<String>(0).map_err(engine)?;
            bytes = bytes.saturating_add(json.len());
            if bytes >= MAX_BYTES || result.len() >= 100_000 {
                return Err(invalid("Memory signals exceed bounds"));
            }
            let mut record: MemoryRecallSignal =
                serde_json::from_str(&json).map_err(|_| invalid("Invalid stored memory signal"))?;
            record.best_score = crate::database::indexed::number(&row, 1)?;
            record.hit_count = crate::database::indexed::number(&row, 2)?;
            result.push(record);
        }
        encoded(&result)?;
        Ok(result)
    }
}
