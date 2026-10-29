use super::envelope;
use crate::{database::Database, services::Result, vault::VaultRoot};
use lapis_app_protocol::database::{
    indexed::records::FileRecord,
    search::{SearchManifestRecord, source::SearchChunkingSettings},
};
use lapis_plugin_services::{markdown::content_hash, search::canvas_text};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Mutex;

pub(super) const PROVIDER: &str = "search:canvas";
pub(super) fn accepts(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("canvas"))
}
pub(super) async fn run(
    database: &Database,
    root: &VaultRoot,
    files: &[String],
    manifest: &mut BTreeMap<String, SearchManifestRecord>,
    chunking: &SearchChunkingSettings,
    progress: &Mutex<super::super::IndexingStatus>,
    cancelled: impl Fn() -> bool,
) -> Result<usize> {
    let mut pending = 0usize;
    let mut failure = None;
    for path in files.iter().filter(|path| accepts(path)) {
        if cancelled() {
            return Ok(pending);
        }
        progress.lock().unwrap_or_else(|e| e.into_inner()).processed += 1;
        let existing = manifest.remove(path);
        if existing
            .as_ref()
            .is_some_and(|row| row.source_provider_id.as_deref() != Some(PROVIDER))
        {
            continue;
        }
        match publish(
            database,
            root,
            path,
            existing.as_ref(),
            chunking,
            &cancelled,
        )
        .await
        {
            Ok(count) => pending = pending.saturating_add(count),
            Err(error) => {
                failure.get_or_insert(error);
            }
        }
    }
    failure.map_or(Ok(pending), Err)
}
async fn publish(
    database: &Database,
    root: &VaultRoot,
    path: &str,
    existing: Option<&SearchManifestRecord>,
    chunking: &SearchChunkingSettings,
    cancelled: &impl Fn() -> bool,
) -> Result<usize> {
    root.verify_physical_root()?;
    let Some(before) = root.stat(path)? else {
        return Ok(1);
    };
    let text = match root.read_text(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(1),
        Err(error) => return Err(error.into()),
    };
    let hash = content_hash(&text);
    if existing.is_some_and(|row| {
        row.metadata_hash.as_deref() == Some(hash.as_str())
            && row.provider_version.as_deref() == Some("1")
            && row.projection_signature.as_deref()
                == Some(envelope::signature(PROVIDER, chunking).as_str())
            && row.source_mtime == Some(before.mtime)
            && row.source_size == Some(before.size as f64)
    }) {
        return Ok(0);
    }
    let file = FileRecord {
        path: path.into(),
        normalized_path: path.into(),
        extension: "canvas".into(),
        mtime: before.mtime,
        size: before.size as f64,
        hash: hash.clone(),
        parser_version: None,
        indexed: false,
        deleted: None,
    };
    let projected = canvas_text(&text);
    let record = envelope::projected(&file, &json!({}), &hash, &projected, PROVIDER, chunking)?;
    root.verify_physical_root()?;
    let Some(after) = root.stat(path)? else {
        return Ok(1);
    };
    if before.mtime != after.mtime || before.size != after.size {
        return Ok(1);
    }
    match root.read_text(path) {
        Ok(current) if current == text => {}
        Ok(_) => return Ok(1),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(1),
        Err(error) => return Err(error.into()),
    }
    if !cancelled() {
        database.upsert_search_document(record).await?;
    }
    Ok(0)
}
