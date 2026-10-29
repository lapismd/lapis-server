use super::{MAX_BYTES, encoded, engine, invalid, reads::parse, store::Store};
use crate::services::Result;
use lapis_app_protocol::database::history::{FileHistory, HistoryFile, HistoryRevision};

impl Store {
    pub async fn history(&self, path: &str) -> Result<Option<FileHistory>> {
        let file_id = {
            let mut rows = self
                .connection
                .query(
                    "SELECT file_id FROM history_file_paths WHERE path=?",
                    [path],
                )
                .await
                .map_err(engine)?;
            let Some(row) = rows.next().await.map_err(engine)? else {
                return Ok(None);
            };
            row.get::<String>(0).map_err(engine)?
        };
        let file: HistoryFile = {
            let mut rows = self
                .connection
                .query(
                    "SELECT data_json FROM history_files WHERE file_id=?",
                    [file_id.as_str()],
                )
                .await
                .map_err(engine)?;
            let Some(row) = rows.next().await.map_err(engine)? else {
                return Ok(None);
            };
            serde_json::from_value(parse(row.get::<String>(0).map_err(engine)?)?)
                .map_err(|_| invalid("Invalid stored history file"))?
        };
        if file.file_id != file_id {
            return Err(invalid("Stored history identity does not match its row"));
        }
        let mut rows = self
            .connection
            .query(
                "SELECT data_json FROM history_revisions WHERE file_id=? ORDER BY ordinal",
                [file_id.as_str()],
            )
            .await
            .map_err(engine)?;
        let mut revisions = Vec::new();
        let mut bytes = 0;
        while let Some(row) = rows.next().await.map_err(engine)? {
            let json = row.get::<String>(0).map_err(engine)?;
            bytes += json.len();
            if bytes >= MAX_BYTES {
                return Err(invalid("History result exceeds 32 MiB"));
            }
            let revision: HistoryRevision = serde_json::from_value(parse(json)?)
                .map_err(|_| invalid("Invalid stored history revision"))?;
            if revision.file_id != file_id {
                return Err(invalid("Stored history revision does not match its file"));
            }
            revisions.push(revision);
        }
        let history = FileHistory { file, revisions };
        encoded(&history)?;
        Ok(Some(history))
    }
}
