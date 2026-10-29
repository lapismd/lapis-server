use crate::{database::Database, services::Result, vault::VaultRoot};
use lapis_app_protocol::database::projections::ProjectionDefinition;
use lapis_plugin_services::markdown::{PARSER_VERSION, content_hash};

pub(super) async fn publish(
    database: &Database,
    root: &VaultRoot,
    definition: &ProjectionDefinition,
    path: &str,
    cancelled: impl Fn() -> bool,
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
    let Some(indexed) = database.get_indexed_file(path).await? else {
        return Ok(1);
    };
    if indexed.file.hash != hash
        || indexed.file.mtime != before.mtime
        || indexed.file.size != before.size as f64
        || !indexed.metadata.as_ref().is_some_and(|metadata| {
            metadata.hash == hash && metadata.parser_version == PARSER_VERSION
        })
    {
        return Ok(1);
    }
    let result = lapis_docs_projection::project_note(path, &text);
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
        super::publication::publish(database, definition, path, &hash, result).await?;
    }
    Ok(0)
}
