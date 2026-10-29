use super::ID;
use crate::database::{
    changes::now_ms, encoded, engine, invalid, projections::Action, store::Store,
};
use crate::services::Result;
use lapis_app_protocol::database::{
    changes::ChangeDomain,
    projections::*,
    tasks::{enums::TaskKind, record::TaskRecord},
};
use serde_json::Value;
impl Store {
    pub(super) async fn put_task(&mut self, row: TaskRecord) -> Result<()> {
        self.ensure_tasks().await?;
        let path = row.document_path.clone();
        self.begin().await?;
        let result=async {
            let json=String::from_utf8(encoded(&row)?).map_err(|_|invalid("Invalid task JSON"))?;
            self.connection.execute("INSERT INTO task_records(document_path,data_json) VALUES(?,?) ON CONFLICT(document_path) DO UPDATE SET data_json=excluded.data_json",turso::params![path.as_str(),json]).await.map_err(engine)?;
            Ok(((),Some(self.journal(ChangeDomain::Task,vec![path.clone()]).await?)))
        }.await;
        self.finish(result).await?;
        let mut rows = self
            .connection
            .query("SELECT hash FROM files WHERE path=?", [path.as_str()])
            .await
            .map_err(engine)?;
        let hash = match rows.next().await.map_err(engine)? {
            Some(r) => r.get(0).map_err(engine)?,
            None => row.document_id.clone(),
        };
        drop(rows);
        let kind = match row.kind {
            TaskKind::Task => "task",
            TaskKind::TaskList => "task-list",
        };
        let Value::Object(data) =
            serde_json::to_value(&row).map_err(|_| invalid("Invalid task JSON"))?
        else {
            unreachable!("task is an object")
        };
        self.projection(Action::Replace(ProjectionSource {
            projection_id: ID.into(),
            source_path: path,
            source_hash: hash,
            rows: vec![ProjectionRow {
                id: row.document_id,
                kind: kind.into(),
                ordinal: None,
                data,
            }],
            edges: vec![],
            writer_plugin_id: None,
        }))
        .await
    }
    pub(super) async fn delete_task(&mut self, path: &str) -> Result<()> {
        self.begin().await?;
        let result = async {
            self.connection
                .execute("DELETE FROM task_records WHERE document_path=?", [path])
                .await
                .map_err(engine)?;
            self.delete_projection(ID, path).await?;
            Ok((
                (),
                Some(
                    self.journal_domains(
                        vec![ChangeDomain::Task, ChangeDomain::Projection],
                        vec![path.into()],
                    )
                    .await?,
                ),
            ))
        }
        .await;
        self.finish(result).await
    }
    async fn ensure_tasks(&mut self) -> Result<()> {
        let mut rows = self
            .connection
            .query(
                "SELECT active FROM index_projections WHERE projection_id=?",
                [ID],
            )
            .await
            .map_err(engine)?;
        if let Some(row) = rows.next().await.map_err(engine)?
            && row.get::<i64>(0).map_err(engine)? != 0
        {
            return Ok(());
        }
        drop(rows);
        self.projection(Action::Register(ProjectionDefinition {
            projection_id: ID.into(),
            owner_plugin_id: "tasks".into(),
            schema_version: 4.0,
            config_hash: String::new(),
            visibility: ProjectionVisibility::Public,
            fields: serde_json::from_str(include_str!("fields.json"))
                .map_err(|_| invalid("Invalid task field schema"))?,
            active: true,
            updated_at: now_ms()? as f64,
        }))
        .await
    }
}
