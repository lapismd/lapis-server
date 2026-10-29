use crate::database::{changes::now_ms, encoded, engine, invalid, store::Store};
use crate::services::Result;
use lapis_app_protocol::database::projections::ProjectionDefinition;

impl Store {
    pub(super) async fn projection_definition(&self, id: &str) -> Result<ProjectionDefinition> {
        let mut rows = self.connection.query("SELECT owner_plugin_id,schema_version,config_hash,visibility,fields_json,active,updated_at FROM index_projections WHERE projection_id=?",[id]).await.map_err(engine)?;
        let row = rows
            .next()
            .await
            .map_err(engine)?
            .ok_or_else(|| invalid("Projection is not registered"))?;
        if row.get::<i64>(5).map_err(engine)? == 0 {
            return Err(invalid("Projection is not registered"));
        }
        let definition = ProjectionDefinition {
            projection_id: id.into(),
            owner_plugin_id: row.get(0).map_err(engine)?,
            schema_version: number(&row, 1)?,
            config_hash: row.get(2).map_err(engine)?,
            visibility: serde_json::from_value(serde_json::Value::String(
                row.get(3).map_err(engine)?,
            ))
            .map_err(|_| invalid("Invalid stored projection visibility"))?,
            fields: serde_json::from_str(&row.get::<String>(4).map_err(engine)?)
                .map_err(|_| invalid("Invalid stored projection fields"))?,
            active: true,
            updated_at: number(&row, 6)?,
        };
        Ok(definition)
    }
    pub(super) async fn register_projection(
        &self,
        definition: &ProjectionDefinition,
    ) -> Result<()> {
        let visibility = match definition.visibility {
            lapis_app_protocol::database::projections::ProjectionVisibility::Public => "public",
            lapis_app_protocol::database::projections::ProjectionVisibility::Private => "private",
        };
        self.connection.execute("INSERT INTO index_projections(projection_id,owner_plugin_id,schema_version,config_hash,visibility,fields_json,active,updated_at) VALUES(?,?,?,?,?,?,?,?) ON CONFLICT(projection_id) DO UPDATE SET owner_plugin_id=excluded.owner_plugin_id,schema_version=excluded.schema_version,config_hash=excluded.config_hash,visibility=excluded.visibility,fields_json=excluded.fields_json,active=excluded.active,updated_at=excluded.updated_at",turso::params![definition.projection_id.as_str(),definition.owner_plugin_id.as_str(),definition.schema_version,definition.config_hash.as_str(),visibility,json(&definition.fields)?,i64::from(definition.active),definition.updated_at]).await.map_err(engine)?;
        Ok(())
    }
    pub(super) async fn unregister_projection(&self, id: &str) -> Result<()> {
        self.connection
            .execute(
                "UPDATE index_projections SET active=0,updated_at=? WHERE projection_id=?",
                turso::params![now_ms()?, id],
            )
            .await
            .map_err(engine)?;
        for table in [
            "index_projection_edges",
            "index_projection_values",
            "index_projection_rows",
            "index_projection_sources",
        ] {
            self.connection
                .execute(&format!("DELETE FROM {table} WHERE projection_id=?"), [id])
                .await
                .map_err(engine)?;
        }
        Ok(())
    }
}
pub(super) fn json(value: &impl serde::Serialize) -> Result<String> {
    String::from_utf8(encoded(value)?).map_err(|_| invalid("Invalid projection JSON"))
}
pub(super) fn number(row: &turso::Row, index: usize) -> Result<f64> {
    match row.get_value(index).map_err(engine)? {
        turso::Value::Integer(n) => Ok(n as f64),
        turso::Value::Real(n) if n.is_finite() => Ok(n),
        _ => Err(invalid("Invalid stored projection number")),
    }
}
