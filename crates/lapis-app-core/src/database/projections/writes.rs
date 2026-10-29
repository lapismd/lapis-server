use super::{Action, definitions::json, values};
use crate::database::{changes::now_ms, engine, store::Store};
use crate::services::Result;
use lapis_app_protocol::database::{
    changes::{ChangeDomain, ChangeSet},
    projections::{ProjectionDefinition, ProjectionSource},
};

impl Store {
    pub(super) async fn write_projection(&self, action: Action) -> Result<((), Option<ChangeSet>)> {
        let paths = match action {
            Action::Register(definition) => {
                self.register_projection(&definition).await?;
                vec![]
            }
            Action::Unregister(id) => {
                self.unregister_projection(&id).await?;
                vec![]
            }
            Action::Replace(input) => {
                let definition = self.projection_definition(&input.projection_id).await?;
                self.delete_projection(&input.projection_id, &input.source_path)
                    .await?;
                self.write_projection_source(
                    &definition,
                    &input.source_path,
                    &input.source_hash,
                    None,
                )
                .await?;
                self.write_projection_rows(&definition, &input).await?;
                vec![input.source_path]
            }
            Action::Error(input) => {
                let definition = self.projection_definition(&input.projection_id).await?;
                self.delete_projection(&input.projection_id, &input.source_path)
                    .await?;
                self.write_projection_source(
                    &definition,
                    &input.source_path,
                    &input.source_hash,
                    Some(&input.error),
                )
                .await?;
                vec![input.source_path]
            }
            Action::Delete(id, path) => {
                self.delete_projection(&id, &path).await?;
                vec![path]
            }
        };
        Ok((
            (),
            Some(self.journal(ChangeDomain::Projection, paths).await?),
        ))
    }
    async fn write_projection_source(
        &self,
        definition: &ProjectionDefinition,
        path: &str,
        hash: &str,
        error: Option<&str>,
    ) -> Result<()> {
        self.connection.execute("INSERT INTO index_projection_sources(projection_id,source_path,source_hash,schema_version,config_hash,status,error,indexed_at) VALUES(?,?,?,?,?,?,?,?)",turso::params![definition.projection_id.as_str(),path,hash,definition.schema_version,definition.config_hash.as_str(),if error.is_some() {"error"} else {"ready"},error,now_ms()?]).await.map_err(engine)?;
        Ok(())
    }
    async fn write_projection_rows(
        &self,
        definition: &ProjectionDefinition,
        input: &ProjectionSource,
    ) -> Result<()> {
        let id = input.projection_id.as_str();
        for row in &input.rows {
            self.connection.execute("INSERT INTO index_projection_rows(projection_id,row_id,source_path,kind,ordinal,data_json) VALUES(?,?,?,?,?,?)",turso::params![id,row.id.as_str(),input.source_path.as_str(),row.kind.as_str(),row.ordinal.unwrap_or(0.0),json(&row.data)?]).await.map_err(engine)?;
            values::write(&self.connection, id, row, definition).await?;
        }
        for edge in &input.edges {
            self.connection.execute("INSERT INTO index_projection_edges(projection_id,source_row_id,relation,target_projection_id,target_row_id,target_path,target_text,ordinal,data_json) VALUES(?,?,?,?,?,?,?,?,?)",turso::params![id,edge.source_row_id.as_str(),edge.relation.as_str(),edge.target_projection_id.as_deref(),edge.target_row_id.as_deref(),edge.target_path.as_deref(),edge.target_text.as_deref(),edge.ordinal,json(&edge.data)?]).await.map_err(engine)?;
        }
        Ok(())
    }
    pub(in crate::database) async fn delete_projection(&self, id: &str, path: &str) -> Result<()> {
        for (table, column) in [
            ("index_projection_edges", "source_row_id"),
            ("index_projection_values", "row_id"),
        ] {
            self.connection.execute(&format!("DELETE FROM {table} WHERE projection_id=? AND {column} IN (SELECT row_id FROM index_projection_rows WHERE projection_id=? AND source_path=?)"),turso::params![id,id,path]).await.map_err(engine)?;
        }
        for table in ["index_projection_rows", "index_projection_sources"] {
            self.connection
                .execute(
                    &format!("DELETE FROM {table} WHERE projection_id=? AND source_path=?"),
                    turso::params![id, path],
                )
                .await
                .map_err(engine)?;
        }
        Ok(())
    }
}
