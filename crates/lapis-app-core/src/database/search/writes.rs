use super::{Action, Value, json, optional_number, optional_text, text};
use crate::database::{engine, store::Store};
use crate::services::Result;
use lapis_app_protocol::database::{changes::ChangeDomain, search::records::SearchDocumentRecord};

impl Store {
    pub(super) async fn mutate_search(&mut self, action: Action) -> Result<Value> {
        self.begin().await?;
        let result = async {
            let path = match action {
                Action::Put(record) => {
                    self.write_search(&record).await?;
                    record.path
                }
                Action::Delete(path) => {
                    self.delete_search_rows(&path).await?;
                    path
                }
                _ => unreachable!("reads handled before transaction"),
            };
            Ok((
                Value::Done,
                Some(self.journal(ChangeDomain::Search, vec![path]).await?),
            ))
        }
        .await;
        self.finish(result).await
    }
    pub(in crate::database) async fn delete_search_rows(&self, path: &str) -> Result<()> {
        for table in ["search_chunks", "search_docs"] {
            self.connection
                .execute(&format!("DELETE FROM {table} WHERE path=?"), [path])
                .await
                .map_err(engine)?;
        }
        Ok(())
    }
    pub(in crate::database) async fn write_search(
        &self,
        record: &SearchDocumentRecord,
    ) -> Result<()> {
        let source = record.source_metadata.as_ref();
        self.connection
            .execute(
                "DELETE FROM search_chunks WHERE path=?",
                [record.path.as_str()],
            )
            .await
            .map_err(engine)?;
        let args = vec![
            text(&record.path),
            optional_text(record.source_provider_id.as_deref()),
            optional_text(source.and_then(|s| s.metadata_hash.as_deref())),
            optional_text(source.and_then(|s| s.provider_version.as_deref())),
            optional_text(source.and_then(|s| s.projection_signature.as_deref())),
            optional_number(source.and_then(|s| s.source_mtime)),
            optional_number(source.and_then(|s| s.source_size)),
            text(&record.name),
            text(&record.extension),
            text(&record.checksum),
            text(&record.content),
            text(record.tags.join(" ")),
            text(record.metadata_text.as_deref().unwrap_or("")),
            text(json(record)?),
        ];
        self.connection.execute("INSERT INTO search_docs(path,source_provider_id,metadata_hash,provider_version,projection_signature,source_mtime,source_size,name,extension,checksum,content,tags,metadata_text,data_json) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(path) DO UPDATE SET source_provider_id=excluded.source_provider_id,metadata_hash=excluded.metadata_hash,provider_version=excluded.provider_version,projection_signature=excluded.projection_signature,source_mtime=excluded.source_mtime,source_size=excluded.source_size,name=excluded.name,extension=excluded.extension,checksum=excluded.checksum,content=excluded.content,tags=excluded.tags,metadata_text=excluded.metadata_text,data_json=excluded.data_json",args).await.map_err(engine)?;
        for chunk in record.chunks.iter().flatten() {
            let vector = chunk
                .embedding
                .as_ref()
                .and_then(|e| e.vector.as_ref())
                .filter(|v| !v.is_empty());
            let mut args = vec![
                text(&chunk.id),
                text(&record.path),
                text(&chunk.text),
                text(json(&chunk.embedding)?),
            ];
            let sql = if let Some(vector) = vector {
                args.push(text(json(vector)?));
                "INSERT INTO search_chunks(id,path,text,embedding_json,embedding) VALUES(?,?,?,?,vector32(?))"
            } else {
                "INSERT INTO search_chunks(id,path,text,embedding_json,embedding) VALUES(?,?,?,?,NULL)"
            };
            self.connection.execute(sql, args).await.map_err(engine)?;
        }
        Ok(())
    }
}
