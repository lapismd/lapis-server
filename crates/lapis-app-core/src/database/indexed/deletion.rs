use crate::database::{
    MAX_BYTES, encoded, engine, invalid, saved_state::notebook_key, store::Store,
};
use crate::services::Result;
use lapis_app_protocol::database::changes::ChangeDomain;

pub(super) fn domains() -> Vec<ChangeDomain> {
    vec![
        ChangeDomain::Metadata,
        ChangeDomain::Search,
        ChangeDomain::Task,
        ChangeDomain::Projection,
        ChangeDomain::Notebook,
    ]
}
impl Store {
    pub(super) async fn delete_indexed(&mut self, path: &str) -> Result<()> {
        self.begin().await?;
        let result = async {
            self.connection.execute("UPDATE files SET indexed=0,deleted=1 WHERE path=?", [path]).await.map_err(engine)?;
            self.delete_indexed_metadata(path).await?;
            self.delete_search_rows(path).await?;
            self.connection.execute("DELETE FROM task_records WHERE document_path=?", [path]).await.map_err(engine)?;
            let mut rows = self.connection.query("SELECT projection_id FROM index_projection_sources WHERE source_path=? UNION SELECT projection_id FROM index_projection_rows WHERE source_path=?", turso::params![path,path]).await.map_err(engine)?;
            let mut ids = vec![];
            let mut bytes = 0usize;
            while let Some(row) = rows.next().await.map_err(engine)? {
                let id = row.get::<String>(0).map_err(engine)?;
                bytes = bytes.saturating_add(id.len());
                if bytes >= MAX_BYTES || ids.len() >= 100_000 { return Err(invalid("Indexed projection cleanup exceeds bounds")); }
                ids.push(id);
            }
            drop(rows);
            for id in ids { self.delete_projection(&id, path).await?; }
            Ok(((), Some(self.journal_domains(domains(), vec![path.into()]).await?)))
        }.await;
        self.finish(result).await
    }
    pub(super) async fn delete_indexed_metadata(&self, path: &str) -> Result<()> {
        self.connection
            .execute("DELETE FROM app_meta WHERE key=?", [notebook_key(path)])
            .await
            .map_err(engine)?;
        for (table, column) in [
            ("metadata", "path"),
            ("links", "source_path"),
            ("tags", "path"),
            ("properties", "path"),
            ("metadata_links", "source_path"),
            ("metadata_tags", "path"),
            ("metadata_tag_ancestors", "path"),
            ("metadata_properties", "path"),
            ("metadata_property_values", "path"),
        ] {
            self.connection
                .execute(&format!("DELETE FROM {table} WHERE {column}=?"), [path])
                .await
                .map_err(engine)?;
        }
        Ok(())
    }
    pub(super) async fn rename_incoming(&self, old: &str, new: &str) -> Result<()> {
        let mut rows = self.connection.query("SELECT source_path,ordinal,data_json FROM metadata_links WHERE resolved_target_path=?", [old]).await.map_err(engine)?;
        let mut incoming = vec![];
        let mut bytes = 0usize;
        while let Some(row) = rows.next().await.map_err(engine)? {
            let source = row.get::<String>(0).map_err(engine)?;
            let ordinal = row.get::<i64>(1).map_err(engine)?;
            let raw = row.get::<String>(2).map_err(engine)?;
            bytes = bytes.saturating_add(raw.len()).saturating_add(source.len());
            if bytes >= MAX_BYTES || incoming.len() >= 100_000 {
                return Err(invalid("Indexed incoming links exceed bounds"));
            }
            let mut link: serde_json::Value =
                serde_json::from_str(&raw).map_err(|_| invalid("Invalid stored incoming link"))?;
            link.as_object_mut()
                .ok_or_else(|| invalid("Invalid stored incoming link"))?
                .insert(
                    "resolvedTargetPath".into(),
                    serde_json::Value::String(new.into()),
                );
            let json = String::from_utf8(encoded(&link)?)
                .map_err(|_| invalid("Invalid incoming link JSON"))?;
            incoming.push((source, ordinal, json));
        }
        drop(rows);
        for (source, ordinal, json) in incoming {
            self.connection.execute("UPDATE metadata_links SET resolved_target_path=?,data_json=? WHERE source_path=? AND ordinal=?", turso::params![new,json.as_str(),source.as_str(),ordinal]).await.map_err(engine)?;
            self.connection
                .execute(
                    "UPDATE links SET data_json=? WHERE source_path=? AND ordinal=?",
                    turso::params![json, source, ordinal],
                )
                .await
                .map_err(engine)?;
        }
        Ok(())
    }
}
