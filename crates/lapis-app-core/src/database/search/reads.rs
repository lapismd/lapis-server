use super::text;
use crate::database::{MAX_BYTES, encoded, engine, indexed, invalid, store::Store};
use crate::services::Result;
use lapis_app_protocol::database::search::{records::SearchDocumentRecord, *};

impl Store {
    pub(in crate::database) async fn search_rows(
        &self,
        sql: &str,
        args: Vec<turso::Value>,
    ) -> Result<Vec<SearchDocumentRecord>> {
        let mut rows = self.connection.query(sql, args).await.map_err(engine)?;
        let mut result = vec![];
        let mut bytes = 0usize;
        while let Some(row) = rows.next().await.map_err(engine)? {
            let value = row.get::<String>(0).map_err(engine)?;
            bytes = bytes.saturating_add(value.len());
            if bytes >= MAX_BYTES || result.len() >= 100_000 {
                return Err(invalid("Search results exceed bounds"));
            }
            result.push(
                serde_json::from_str(&value)
                    .map_err(|_| invalid("Invalid stored Search document"))?,
            );
        }
        encoded(&result)?;
        Ok(result)
    }
    pub(super) async fn search_manifest(
        &self,
        query: SearchManifestQuery,
    ) -> Result<SearchManifestPage> {
        let count = indexed::limit(query.limit, 500, 1)?;
        let mut rows=self.connection.query("SELECT path,checksum,source_provider_id,metadata_hash,provider_version,projection_signature,source_mtime,source_size FROM search_docs WHERE path>? ORDER BY path LIMIT ?",vec![text(query.after.unwrap_or_default()),turso::Value::Integer((count+1) as i64)]).await.map_err(engine)?;
        let mut result = vec![];
        let mut next_cursor = None;
        let mut bytes = 0usize;
        while let Some(row) = rows.next().await.map_err(engine)? {
            if result.len() == count {
                next_cursor = result.last().map(|r: &SearchManifestRecord| r.path.clone());
                break;
            }
            let record = SearchManifestRecord {
                path: row.get(0).map_err(engine)?,
                checksum: row.get(1).map_err(engine)?,
                source_provider_id: indexed::optional_text(&row, 2)?,
                metadata_hash: indexed::optional_text(&row, 3)?,
                provider_version: indexed::optional_text(&row, 4)?,
                projection_signature: indexed::optional_text(&row, 5)?,
                source_mtime: optional_number(&row, 6)?,
                source_size: optional_number(&row, 7)?,
            };
            bytes = bytes.saturating_add(encoded(&record)?.len());
            if bytes >= MAX_BYTES || result.len() >= 100_000 {
                return Err(invalid("Search manifest exceeds bounds"));
            }
            result.push(record);
        }
        let page = SearchManifestPage {
            rows: result,
            next_cursor,
        };
        encoded(&page)?;
        Ok(page)
    }
    pub(super) async fn search_stats(&self) -> Result<SearchIndexStats> {
        let mut count = self
            .connection
            .query("SELECT count(*) FROM search_docs", ())
            .await
            .map_err(engine)?;
        let document_count = count
            .next()
            .await
            .map_err(engine)?
            .ok_or_else(|| invalid("Missing Search count"))?
            .get::<i64>(0)
            .map_err(engine)?;
        drop(count);
        let mut result = SearchIndexStats {
            document_count: document_count as usize,
            ..Default::default()
        };
        let mut rows = self
            .connection
            .query("SELECT embedding_json FROM search_chunks", ())
            .await
            .map_err(engine)?;
        while let Some(row) = rows.next().await.map_err(engine)? {
            result.chunk_count += 1;
            let embedding = indexed::optional_text(&row, 0)?
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
            match embedding
                .as_ref()
                .and_then(|e| e.get("status"))
                .and_then(|v| v.as_str())
            {
                Some("ready") => result.ready_chunk_count += 1,
                Some("error") => {
                    result.error_chunk_count += 1;
                    if result.last_error.is_none() {
                        result.last_error = embedding
                            .as_ref()
                            .and_then(|e| e.get("error"))
                            .and_then(|v| v.as_str())
                            .map(str::to_owned);
                    }
                }
                _ => result.pending_chunk_count += 1,
            }
        }
        encoded(&result)?;
        Ok(result)
    }
}
fn optional_number(row: &turso::Row, index: usize) -> Result<Option<f64>> {
    if matches!(row.get_value(index).map_err(engine)?, turso::Value::Null) {
        Ok(None)
    } else {
        indexed::number(row, index).map(Some)
    }
}
