use super::ID;
use crate::database::{MAX_BYTES, encoded, engine, invalid, store::Store};
use crate::services::Result;
use lapis_app_protocol::database::{
    indexed::records::LinkRecord,
    tasks::{TaskChildQuery, TaskLookup},
};
use serde_json::Value;
use std::collections::{HashSet, VecDeque};
impl Store {
    pub(in crate::database) async fn task_row(&self, lookup: TaskLookup) -> Result<Option<Value>> {
        if let Some(id) = lookup.id.filter(|s| !s.is_empty()) {
            return self.projection_row(ID, &id, None).await;
        }
        let Some(path) = lookup.path.filter(|s| !s.is_empty()) else {
            return Ok(None);
        };
        if self.native_tasks_selected().await? {
            let mut row = self.projection_row(ID, &path, None).await?;
            if let Some(Value::Object(row)) = &mut row {
                row.remove("id");
            }
            return Ok(row);
        }
        let mut rows = self
            .connection
            .query(
                "SELECT data_json FROM task_records WHERE document_path=?",
                [path],
            )
            .await
            .map_err(engine)?;
        let Some(row) = rows.next().await.map_err(engine)? else {
            return Ok(None);
        };
        let json = row.get::<String>(0).map_err(engine)?;
        if json.len() >= MAX_BYTES {
            return Err(invalid("Task row exceeds 32 MiB"));
        }
        Ok(serde_json::from_str(&json).ok())
    }
    pub(super) async fn task_children(&self, query: TaskChildQuery) -> Result<Vec<LinkRecord>> {
        if self.native_tasks_selected().await? {
            return self.native_task_children(query).await;
        }
        let mut args = vec![query.source_path];
        let filter = if let Some(kind) = query.kind {
            args.push(
                serde_json::to_value(kind)
                    .map_err(|_| invalid("Invalid link kind"))?
                    .as_str()
                    .expect("link kind")
                    .to_owned(),
            );
            " AND link_kind=?"
        } else {
            ""
        };
        let mut rows=self.connection.query(&format!("SELECT data_json FROM metadata_links WHERE source_path=?{filter} ORDER BY ordinal"),args).await.map_err(engine)?;
        let mut result = vec![];
        let mut bytes = 0usize;
        while let Some(row) = rows.next().await.map_err(engine)? {
            let json = row.get::<String>(0).map_err(engine)?;
            bytes = bytes.saturating_add(json.len());
            if bytes >= MAX_BYTES || result.len() >= 100_000 {
                return Err(invalid("Task links exceed result bounds"));
            }
            result.push(
                serde_json::from_str(&json).map_err(|_| invalid("Invalid stored task link"))?,
            );
        }
        encoded(&result)?;
        Ok(result)
    }
    pub(super) async fn task_descendants(&self, path: &str) -> Result<Vec<Value>> {
        let mut rows=self.connection.query("SELECT row_id FROM index_projection_rows WHERE projection_id=? AND source_path=? LIMIT 1",turso::params![ID,path]).await.map_err(engine)?;
        let Some(row) = rows.next().await.map_err(engine)? else {
            return Ok(vec![]);
        };
        let first = row.get::<String>(0).map_err(engine)?;
        drop(rows);
        let mut seen = HashSet::from([first.clone()]);
        let mut queue = VecDeque::from([first]);
        let mut result = vec![];
        let mut bytes = 0usize;
        while let Some(source) = queue.pop_front() {
            let mut edges=self.connection.query("SELECT target_row_id,target_path FROM index_projection_edges WHERE projection_id=? AND source_row_id=? AND relation IN ('task-entry','list-entry') ORDER BY ordinal",turso::params![ID,source]).await.map_err(engine)?;
            while let Some(edge) = edges.next().await.map_err(engine)? {
                let id =
                    crate::database::indexed::optional_text(&edge, 0)?.filter(|s| !s.is_empty());
                let path =
                    crate::database::indexed::optional_text(&edge, 1)?.filter(|s| !s.is_empty());
                let Some(row) = self.task_row(TaskLookup { id, path }).await? else {
                    continue;
                };
                let Some(id) = row.get("documentId").and_then(Value::as_str) else {
                    return Err(invalid("Stored task has no document identity"));
                };
                if !seen.insert(id.to_owned()) {
                    continue;
                }
                bytes = bytes
                    .saturating_add(encoded(&row)?.len())
                    .saturating_add(id.len().saturating_mul(2));
                if bytes >= MAX_BYTES || result.len() >= 100_000 {
                    return Err(invalid("Task descendants exceed result bounds"));
                }
                queue.push_back(id.to_owned());
                result.push(row);
            }
        }
        encoded(&result)?;
        Ok(result)
    }
}
