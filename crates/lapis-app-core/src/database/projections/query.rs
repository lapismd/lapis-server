use super::{query_page::Page, values::js_string};
use crate::database::{MAX_BYTES, engine, indexed::optional_text, invalid, store::Store};
use crate::services::Result;
use lapis_app_protocol::database::projections::queries::*;
use serde_json::{Map, Value};

impl Store {
    pub(in crate::database) async fn query_projection(
        &self,
        id: &str,
        query: ProjectionQuery,
        reader: Option<&str>,
    ) -> Result<ProjectionQueryResult> {
        let definition = self.readable_projection(id, reader).await?;
        let freshness = if query.include_stale {
            ""
        } else {
            "AND s.status='ready' AND (f.path IS NULL OR (f.hash=s.source_hash AND f.deleted=0))"
        };
        let mut rows = self.connection.query(&format!("SELECT r.row_id,r.data_json FROM index_projection_rows r INNER JOIN index_projection_sources s ON s.projection_id=r.projection_id AND s.source_path=r.source_path LEFT JOIN files f ON f.path=r.source_path WHERE r.projection_id=? AND s.schema_version=? AND s.config_hash=? {freshness} ORDER BY r.row_id"),turso::params![id,definition.schema_version,definition.config_hash]).await.map_err(engine)?;
        let mut page = Page::new(query)?;
        while let Some(row) = rows.next().await.map_err(engine)? {
            let json = row.get::<String>(1).map_err(engine)?;
            if json.len() >= MAX_BYTES {
                return Err(invalid("Projection row exceeds 32 MiB"));
            }
            let data: Map<String, Value> = serde_json::from_str(&json).unwrap_or_default();
            page.push(row.get(0).map_err(engine)?, data)?;
        }
        drop(rows);
        let mut status = self.connection.query("SELECT COALESCE(MAX(CASE status WHEN 'building' THEN 2 WHEN 'error' THEN 1 ELSE 0 END),0) FROM index_projection_sources WHERE projection_id=?",[id]).await.map_err(engine)?;
        let status = match status
            .next()
            .await
            .map_err(engine)?
            .ok_or_else(|| invalid("Missing projection status"))?
            .get::<i64>(0)
            .map_err(engine)?
        {
            2 => ProjectionIndexStatus::Building,
            1 => ProjectionIndexStatus::Error,
            _ => ProjectionIndexStatus::Ready,
        };
        page.finish(self.revision().await?, status)
    }
    pub(super) async fn query_related(
        &self,
        query: ProjectionRelatedQuery,
        reader: Option<&str>,
    ) -> Result<ProjectionQueryResult> {
        self.readable_projection(&query.projection_id, reader)
            .await?;
        let incoming = matches!(query.direction, Some(ProjectionRelationDirection::In));
        let sql = if incoming {
            "SELECT source_row_id,target_projection_id,target_row_id,target_path FROM index_projection_edges WHERE projection_id=? AND relation=? AND (target_row_id=? OR target_path=(SELECT source_path FROM index_projection_rows WHERE projection_id=? AND row_id=?)) ORDER BY ordinal"
        } else {
            "SELECT source_row_id,target_projection_id,target_row_id,target_path FROM index_projection_edges WHERE projection_id=? AND relation=? AND source_row_id=? ORDER BY ordinal"
        };
        let mut args = vec![
            query.projection_id.clone(),
            query.relation,
            query.row_id.clone(),
        ];
        if incoming {
            args.extend([query.projection_id.clone(), query.row_id]);
        }
        let mut edges = self.connection.query(sql, args).await.map_err(engine)?;
        let mut page = Page::new(ProjectionQuery {
            r#where: query.target_where,
            limit: query.limit,
            ..Default::default()
        })?;
        while let Some(edge) = edges.next().await.map_err(engine)? {
            let (id, target, path) = if incoming {
                (
                    query.projection_id.clone(),
                    Some(edge.get::<String>(0).map_err(engine)?),
                    None,
                )
            } else {
                (
                    optional_text(&edge, 1)?.unwrap_or_else(|| query.projection_id.clone()),
                    optional_text(&edge, 2)?.filter(|s| !s.is_empty()),
                    optional_text(&edge, 3)?.filter(|s| !s.is_empty()),
                )
            };
            let Some(key) = target.as_ref().or(path.as_ref()) else {
                continue;
            };
            let Some(Value::Object(data)) = self.projection_row(&id, key, reader).await? else {
                continue;
            };
            let row_id = data
                .get("id")
                .filter(|v| !v.is_null())
                .or_else(|| data.get("documentId").filter(|v| !v.is_null()))
                .map(js_string)
                .unwrap_or_else(|| key.clone());
            page.push(row_id, data)?;
        }
        page.finish(self.revision().await?, ProjectionIndexStatus::Ready)
    }
}
