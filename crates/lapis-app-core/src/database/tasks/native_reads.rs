use super::{ID, NATIVE_CONFIG};
use crate::{
    database::{MAX_BYTES, encoded, engine, indexed::optional_text, invalid, store::Store},
    services::Result,
};
use lapis_app_protocol::database::{
    indexed::records::{LinkKind, LinkRecord, LinkType},
    tasks::TaskChildQuery,
};
use serde_json::Value;

impl Store {
    pub(super) async fn native_tasks_selected(&self) -> Result<bool> {
        let mut rows = self
            .connection
            .query(
                "SELECT config_hash FROM index_projections WHERE projection_id=? AND active=1",
                [ID],
            )
            .await
            .map_err(engine)?;
        Ok(match rows.next().await.map_err(engine)? {
            Some(row) => row.get::<String>(0).map_err(engine)? == NATIVE_CONFIG,
            None => false,
        })
    }
    pub(super) async fn native_task_children(
        &self,
        query: TaskChildQuery,
    ) -> Result<Vec<LinkRecord>> {
        let Some(source) = self.projection_row(ID, &query.source_path, None).await? else {
            return Ok(vec![]);
        };
        let id = source["documentId"]
            .as_str()
            .ok_or_else(|| invalid("Stored task has no document identity"))?;
        let mut rows=self.connection.query("SELECT relation,target_path,target_text,ordinal,data_json FROM index_projection_edges WHERE projection_id=? AND source_row_id=? ORDER BY ordinal",turso::params![ID,id]).await.map_err(engine)?;
        let expected = query
            .kind
            .map(serde_json::to_value)
            .transpose()
            .map_err(|_| invalid("Invalid link kind"))?;
        let mut result = vec![];
        let mut bytes = 0usize;
        while let Some(row) = rows.next().await.map_err(engine)? {
            let relation = row.get::<String>(0).map_err(engine)?;
            if expected
                .as_ref()
                .is_some_and(|kind| kind.as_str() != Some(&relation))
            {
                continue;
            }
            let kind: LinkKind = serde_json::from_value(Value::String(relation))
                .map_err(|_| invalid("Invalid stored task link"))?;
            let data = optional_text(&row, 4)?
                .map(|json| serde_json::from_str::<Value>(&json))
                .transpose()
                .map_err(|_| invalid("Invalid stored task link data"))?
                .unwrap_or(Value::Null);
            let link = LinkRecord {
                source_path: query.source_path.clone(),
                target_text: row.get(2).map_err(engine)?,
                original: None,
                resolved_target_path: optional_text(&row, 1)?,
                link_type: LinkType::Link,
                position: None,
                count: 1.0,
                heading: Some(
                    data.get("headingText")
                        .or_else(|| data.get("section"))
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                ),
                kind: Some(kind),
                ordinal: Some(row.get::<i64>(3).map_err(engine)? as f64),
            };
            bytes = bytes.saturating_add(encoded(&link)?.len());
            if bytes >= MAX_BYTES || result.len() >= 100_000 {
                return Err(invalid("Task links exceed result bounds"));
            }
            result.push(link);
        }
        encoded(&result)?;
        Ok(result)
    }
}
