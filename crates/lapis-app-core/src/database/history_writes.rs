use super::{encoded, engine, invalid, store::Store};
use crate::services::Result;
use lapis_app_protocol::database::{
    changes::{ChangeDomain, ChangeSet},
    history::{HistoryEvent, HistoryFile, HistoryRevision, StoreHistory, StoredHistory},
};
use uuid::Uuid;

#[derive(serde::Serialize)]
struct HistoryBody<'a> {
    file: &'a HistoryFile,
    revisions: &'a [HistoryRevision],
}

impl Store {
    pub async fn store_history(&mut self, input: StoreHistory) -> Result<StoredHistory> {
        self.begin().await?;
        let result = self.apply_history(input).await;
        self.finish(result).await
    }
    async fn apply_history(
        &self,
        input: StoreHistory,
    ) -> Result<(StoredHistory, Option<ChangeSet>)> {
        let mut existing = self.history(&input.path).await?;
        if existing.is_none()
            && let Some(previous) = &input.previous_path
        {
            existing = self.history(previous).await?;
        }
        let file_id = existing
            .as_ref()
            .map(|h| h.file.file_id.clone())
            .unwrap_or_else(|| format!("history-file:{}", Uuid::new_v4()));
        let latest = existing.as_ref().and_then(|h| h.revisions.last());
        let content = input
            .content
            .or_else(|| latest.map(|r| r.content.clone()))
            .unwrap_or_default();
        let content_hash = input
            .content_hash
            .or_else(|| latest.map(|r| r.content_hash.clone()))
            .unwrap_or_else(|| hash_text(&content));
        if !matches!(
            input.event_type,
            HistoryEvent::Rename | HistoryEvent::Delete
        ) && existing.as_ref().is_some_and(|h| !h.file.deleted)
            && latest.is_some_and(|r| r.content_hash == content_hash)
        {
            return Ok((
                StoredHistory {
                    file_id,
                    stored: false,
                    deduplicated: true,
                    revision: None,
                },
                None,
            ));
        }
        let replace = input.replace_latest && latest.is_some();
        let revision_id = latest
            .filter(|_| replace)
            .map(|r| r.revision_id.clone())
            .unwrap_or_else(|| format!("history-revision:{}", Uuid::new_v4()));
        let file = HistoryFile {
            file_id: file_id.clone(),
            current_path: input.path.clone(),
            deleted: input.event_type == HistoryEvent::Delete,
        };
        let revision = HistoryRevision {
            revision_id,
            file_id: file_id.clone(),
            current_path: input.path.clone(),
            captured_path: if input.event_type == HistoryEvent::Rename {
                input.previous_path.unwrap_or_else(|| input.path.clone())
            } else {
                input.path.clone()
            },
            event_type: input.event_type,
            created_at: input.created_at,
            source_mtime: input.source_mtime,
            source_size: input.source_size,
            content_hash,
            content,
        };
        let mut revisions = existing.map(|h| h.revisions).unwrap_or_default();
        if replace {
            revisions.pop();
        }
        revisions.push(revision.clone());
        let limit = input
            .max_revisions
            .unwrap_or(revisions.len() as f64)
            .max(1.0)
            .floor() as usize;
        revisions.drain(..revisions.len().saturating_sub(limit));
        encoded(&HistoryBody {
            file: &file,
            revisions: &revisions,
        })?;
        self.connection.execute("INSERT INTO history_files(file_id,data_json) VALUES(?,?) ON CONFLICT(file_id) DO UPDATE SET data_json=excluded.data_json",turso::params![file_id.as_str(),json(&file)?]).await.map_err(engine)?;
        self.connection
            .execute(
                "DELETE FROM history_revisions WHERE file_id=?",
                [file_id.as_str()],
            )
            .await
            .map_err(engine)?;
        self.connection
            .execute(
                "DELETE FROM history_file_paths WHERE file_id=?",
                [file_id.as_str()],
            )
            .await
            .map_err(engine)?;
        self.connection.execute("INSERT INTO history_file_paths(path,file_id) VALUES(?,?) ON CONFLICT(path) DO UPDATE SET file_id=excluded.file_id",turso::params![input.path.as_str(),file_id.as_str()]).await.map_err(engine)?;
        for (ordinal, entry) in revisions.iter().enumerate() {
            self.connection
                .execute(
                    "INSERT INTO history_revisions(file_id,ordinal,data_json) VALUES(?,?,?)",
                    turso::params![file_id.as_str(), ordinal as i64, json(entry)?],
                )
                .await
                .map_err(engine)?;
        }
        let change = self
            .journal(ChangeDomain::History, vec![input.path])
            .await?;
        Ok((
            StoredHistory {
                file_id,
                stored: true,
                deduplicated: false,
                revision: Some(revision),
            },
            Some(change),
        ))
    }
}
fn json(value: &impl serde::Serialize) -> Result<String> {
    String::from_utf8(encoded(value)?).map_err(|_| invalid("Invalid history JSON"))
}
fn hash_text(text: &str) -> String {
    format!(
        "{:08x}",
        text.chars()
            .fold(2166136261_u32, |hash, c| (hash ^ c as u32)
                .wrapping_mul(16777619))
    )
}
