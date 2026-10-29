use super::envelope;
use crate::{database::Database, services::Result, vault::VaultRoot};
use lapis_app_protocol::database::{
    indexed::records::FileRecord,
    search::{SearchManifestRecord, source::SearchChunkingSettings},
};
use lapis_plugin_services::markdown::{PARSER_VERSION, content_hash};

pub(super) fn unchanged(
    file: &FileRecord,
    row: &SearchManifestRecord,
    chunking: &SearchChunkingSettings,
) -> bool {
    row.source_provider_id.as_deref() == Some(envelope::PROVIDER)
        && row.metadata_hash.as_deref() == Some(file.hash.as_str())
        && row.provider_version.as_deref() == Some("1")
        && row.projection_signature.as_deref()
            == Some(envelope::signature(envelope::PROVIDER, chunking).as_str())
        && row.source_mtime == Some(file.mtime)
        && row.source_size == Some(file.size)
}

pub(super) async fn publish(
    database: &Database,
    root: &VaultRoot,
    file: &FileRecord,
    chunking: &SearchChunkingSettings,
    cancelled: impl Fn() -> bool,
) -> Result<usize> {
    root.verify_physical_root()?;
    let Some(before) = root.stat(&file.path)? else {
        return Ok(1);
    };
    if before.mtime != file.mtime || before.size as f64 != file.size {
        return Ok(1);
    }
    let text = match root.read_text(&file.path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(1),
        Err(error) => return Err(error.into()),
    };
    if content_hash(&text) != file.hash {
        return Ok(1);
    }
    let Some(indexed) = database.get_indexed_file(&file.path).await? else {
        return Ok(1);
    };
    let Some(metadata) = indexed.metadata else {
        return Ok(1);
    };
    if metadata.hash != file.hash || metadata.parser_version != PARSER_VERSION {
        return Ok(1);
    }
    let record = envelope::record(file, &metadata, &text, chunking)?;
    root.verify_physical_root()?;
    let Some(after) = root.stat(&file.path)? else {
        return Ok(1);
    };
    if before.mtime != after.mtime || before.size != after.size {
        return Ok(1);
    }
    match root.read_text(&file.path) {
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
