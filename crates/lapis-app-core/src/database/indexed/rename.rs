use super::deletion::domains;
use crate::database::{encoded, engine, invalid, saved_state::notebook_key, store::Store};
use crate::services::Result;
use lapis_app_protocol::database::{
    changes::ChangedPath,
    indexed::records::{CoreIndexedFile, MetadataRecord},
    tasks::TaskLookup,
};

impl Store {
    pub(super) async fn rename_indexed(&mut self, old: &str, new: &str) -> Result<()> {
        self.begin().await?;
        let result = async {
            let mut rows = self.read_indexed(&[old.into()], None, false).await?;
            let Some(mut row) = rows.pop() else { return Ok(((), None)); };
            let notebook = self.get(&notebook_key(old)).await?.filter(|v| !v.is_null());
            let task = self.task_row(TaskLookup { id:None, path:Some(old.into()) }).await?;
            let mut search = self.search_rows("SELECT data_json FROM search_docs WHERE path=?", vec![super::text(old)]).await?.pop();
            // Update incoming JSON before moving outgoing rows, retaining self-links too.
            self.rename_incoming(old, new).await?;
            for link in &mut row.links {
                link.source_path = new.into();
                if link.resolved_target_path.as_deref() == Some(old) { link.resolved_target_path = Some(new.into()); }
            }
            for tag in &mut row.tags { tag.path = new.into(); }
            for property in &mut row.properties { property.path = new.into(); }
            row.file.path = new.into(); row.file.normalized_path = new.into();
            let mut metadata = row.metadata.unwrap_or(MetadataRecord { path:new.into(), hash:row.file.hash.clone(), parser_version:"unknown".into(), metadata:serde_json::json!({}) });
            metadata.path = new.into();
            self.delete_indexed_metadata(old).await?;
            self.connection.execute("DELETE FROM files WHERE path=?", [old]).await.map_err(engine)?;
            self.write_indexed_record(&CoreIndexedFile { file:row.file, metadata, links:row.links, tags:row.tags, properties:row.properties }).await?;
            for table in ["index_projection_sources","index_projection_rows"] {
                self.connection.execute(&format!("UPDATE {table} SET source_path=? WHERE source_path=?"), turso::params![new,old]).await.map_err(engine)?;
            }
            self.connection.execute("UPDATE index_projection_edges SET target_path=? WHERE target_path=?", turso::params![new,old]).await.map_err(engine)?;
            if let Some(mut notebook) = notebook {
                notebook.as_object_mut().ok_or_else(||invalid("Invalid stored notebook state"))?.insert("sourcePath".into(),serde_json::Value::String(new.into()));
                self.connection.execute("INSERT INTO app_meta(key,value_json) VALUES(?,?) ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json", turso::params![notebook_key(new),String::from_utf8(encoded(&notebook)?).map_err(|_|invalid("Invalid notebook JSON"))?]).await.map_err(engine)?;
            }
            if let Some(record) = &mut search {
                record.path = new.into();
                for chunk in record.chunks.iter_mut().flatten() {
                    if chunk.id == old { chunk.id = new.into(); }
                    else if let Some(suffix) = chunk.id.strip_prefix(&format!("{old}#")) { chunk.id = format!("{new}#{suffix}"); }
                }
                self.delete_search_rows(old).await?;
                self.write_search(record).await?;
            }
            if let Some(mut task) = task {
                task.as_object_mut().ok_or_else(||invalid("Invalid stored task state"))?.insert("documentPath".into(),serde_json::Value::String(new.into()));
                self.connection.execute("DELETE FROM task_records WHERE document_path=?", [old]).await.map_err(engine)?;
                self.connection.execute("INSERT INTO task_records(document_path,data_json) VALUES(?,?) ON CONFLICT(document_path) DO UPDATE SET data_json=excluded.data_json", turso::params![new,String::from_utf8(encoded(&task)?).map_err(|_|invalid("Invalid task JSON"))?]).await.map_err(engine)?;
            }
            let paths = if old == new { vec![old.into()] } else { vec![old.into(),new.into()] };
            Ok(((),Some(self.journal_details(domains(),paths,Some(vec![ChangedPath {old_path:old.into(),new_path:new.into()}])).await?)))
        }.await;
        self.finish(result).await
    }
}
