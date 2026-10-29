use super::{normalize, write_domains};
use crate::database::{encoded, engine, invalid, store::Store};
use crate::services::Result;
use lapis_app_protocol::database::{changes::ChangeDomain, indexed::records::CoreIndexedFile};

impl Store {
    pub async fn put_indexed(&mut self, record: CoreIndexedFile) -> Result<()> {
        self.begin().await?;
        let result = async {
            self.write_indexed_record(&record).await?;
            Ok((
                (),
                Some(
                    self.journal(ChangeDomain::Metadata, vec![record.file.path.clone()])
                        .await?,
                ),
            ))
        }
        .await;
        self.finish(result).await
    }
    pub(super) async fn write_indexed_record(&self, record: &CoreIndexedFile) -> Result<()> {
        let file = &record.file;
        let metadata = &record.metadata;
        self.connection.execute("INSERT INTO files(path,normalized_path,extension,mtime,size,hash,indexed,deleted) VALUES(?,?,?,?,?,?,?,0) ON CONFLICT(path) DO UPDATE SET normalized_path=excluded.normalized_path,extension=excluded.extension,mtime=excluded.mtime,size=excluded.size,hash=excluded.hash,indexed=excluded.indexed,deleted=0",turso::params![file.path.as_str(),file.normalized_path.as_str(),normalize::extension(&file.extension),file.mtime,file.size,file.hash.as_str(),i64::from(file.indexed)]).await.map_err(engine)?;
        self.connection.execute("INSERT INTO metadata(path,hash,parser_version,data_json) VALUES(?,?,?,?) ON CONFLICT(path) DO UPDATE SET hash=excluded.hash,parser_version=excluded.parser_version,data_json=excluded.data_json",turso::params![metadata.path.as_str(),metadata.hash.as_str(),metadata.parser_version.as_str(),json(&metadata.metadata)?]).await.map_err(engine)?;
        for (table, column) in [
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
                .execute(
                    &format!("DELETE FROM {table} WHERE {column}=?"),
                    [file.path.as_str()],
                )
                .await
                .map_err(engine)?;
        }
        write_domains::links(&self.connection, &file.path, &record.links).await?;
        write_domains::tags(&self.connection, &file.path, &record.tags).await?;
        write_domains::properties(&self.connection, &file.path, &record.properties).await?;
        Ok(())
    }
}
pub(super) fn json(value: &impl serde::Serialize) -> Result<String> {
    String::from_utf8(encoded(value)?).map_err(|_| invalid("Invalid indexed JSON"))
}
pub(super) fn enum_text(value: &impl serde::Serialize) -> Result<String> {
    match serde_json::to_value(value).map_err(|_| invalid("Invalid indexed enum"))? {
        serde_json::Value::String(s) => Ok(s),
        _ => Err(invalid("Invalid indexed enum")),
    }
}
