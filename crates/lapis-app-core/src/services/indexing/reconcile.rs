use super::{
    IndexingStatus, TrackedTaskProvider, docs, manifest, occurrences, records, search, tasks,
};
use crate::{
    database::Database,
    services::{Error, Result, VaultService},
    vault::VaultRoot,
};
use lapis_app_protocol::{
    ErrorCode, database::indexed::queries::ManifestQuery, indexing::IndexingPhase,
};
use std::sync::{
    Mutex, Weak,
    atomic::{AtomicBool, Ordering},
};

pub(super) struct Selection<'a> {
    pub occurrences: &'a AtomicBool,
    pub documents: &'a AtomicBool,
    pub search: &'a search::Selection,
    pub docs: &'a Mutex<Option<String>>,
    pub tracked: &'a Mutex<Option<TrackedTaskProvider>>,
    pub today: &'a str,
    pub force: bool,
    pub progress: &'a Mutex<IndexingStatus>,
}
pub(super) async fn run(
    vault: &Weak<VaultService>,
    database: &Database,
    root: &VaultRoot,
    stop: &AtomicBool,
    selection: &Selection<'_>,
) -> IndexingStatus {
    let mut status = IndexingStatus::default();
    if let Err(error) = process(vault, database, root, stop, selection, &mut status).await {
        status.error = Some(error.to_string());
    }
    selection
        .progress
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .phase = IndexingPhase::Docs;
    match docs::run(vault, database, root, stop, selection.docs).await {
        Ok(pending) => status.pending = status.pending.saturating_add(pending),
        Err(error) => {
            status.error.get_or_insert_with(|| error.to_string());
        }
    }
    selection
        .progress
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .phase = IndexingPhase::Search;
    match search::run(
        vault,
        database,
        root,
        stop,
        selection.search,
        selection.progress,
        selection.force,
    )
    .await
    {
        Ok(pending) => status.pending = status.pending.saturating_add(pending),
        Err(error) => {
            status.error.get_or_insert_with(|| error.to_string());
        }
    }
    status
}
async fn process(
    vault: &Weak<VaultService>,
    database: &Database,
    root: &VaultRoot,
    stop: &AtomicBool,
    selection: &Selection<'_>,
    status: &mut IndexingStatus,
) -> Result<()> {
    let vault = vault.upgrade().ok_or_else(Error::closed)?;
    let work = vault.begin_work()?;
    let task_selection = selection.occurrences.load(Ordering::Acquire);
    if task_selection {
        occurrences::register(database).await?;
    }
    let declared = match root.read_text(".lapis/types.json") {
        Ok(text) => serde_json::from_str::<serde_json::Value>(&text).map_err(|_| {
            Error::new(
                ErrorCode::InvalidRequest,
                "Property type configuration is invalid",
            )
        })?["types"]
            .as_object()
            .cloned()
            .unwrap_or_default(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Default::default(),
        Err(error) => return Err(error.into()),
    };
    let Some(files) = manifest::collect(root, || {
        stop.load(Ordering::Acquire) || work.cancellation_requested()
    })?
    else {
        return Ok(());
    };
    {
        let mut progress = selection.progress.lock().unwrap_or_else(|e| e.into_inner());
        progress.phase = IndexingPhase::Metadata;
        progress.total = files.iter().filter(|path| records::markdown(path)).count();
    }
    let mut after = None;
    loop {
        let page = database
            .list_indexed_file_manifest(ManifestQuery {
                after,
                limit: Some(500),
                ..Default::default()
            })
            .await?;
        for row in page.rows {
            if stop.load(Ordering::Acquire) || work.cancellation_requested() {
                return Ok(());
            }
            // Other registered processors retain their own derived records.
            if records::markdown(&row.path) && files.binary_search(&row.path).is_err() {
                root.verify_physical_root()?;
                // A path created after enumeration is not an absent source.
                if root.stat(&row.path)?.is_none() {
                    database.delete_indexed_file(&row.path).await?;
                    status.deleted += 1;
                }
            }
        }
        after = page.next_cursor;
        if after.is_none() {
            break;
        }
    }
    for path in files.iter().filter(|path| records::markdown(path)) {
        if stop.load(Ordering::Acquire) || work.cancellation_requested() {
            return Ok(());
        }
        selection
            .progress
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .processed += 1;
        root.verify_physical_root()?;
        let Some(before) = root.stat(path)? else {
            continue;
        };
        let text = match root.read_text(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        let record = records::record(path, &before, &text, &files, &declared);
        let occurrences = task_selection.then(|| occurrences::prepare(path, &text));
        let source_hash = record.file.hash.clone();
        root.verify_physical_root()?;
        let Some(after) = root.stat(path)? else {
            continue;
        };
        if before.mtime != after.mtime || before.size != after.size {
            status.pending += 1;
            continue;
        }
        // External writers need not advance size or timestamp. Verify the
        // bytes again after parsing, then let watcher/periodic repair cover
        // edits racing the final derived-database publication.
        match root.read_text(path) {
            Ok(current) if current == text => {}
            Ok(_) => {
                status.pending += 1;
                continue;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        }
        let existing = database.get_indexed_file(path).await?;
        let same = existing.as_ref().is_some_and(|row| {
            row.file.hash == record.file.hash
                && row.file.mtime == record.file.mtime
                && row.file.size == record.file.size
                && row.metadata.as_ref().is_some_and(|metadata| {
                    metadata.parser_version == record.metadata.parser_version
                })
                && serde_json::to_value(&row.links).ok() == serde_json::to_value(&record.links).ok()
                && serde_json::to_value(&row.properties).ok()
                    == serde_json::to_value(&record.properties).ok()
        });
        if selection.force || !same {
            database.upsert_indexed_metadata(record).await?;
            status.indexed += 1;
        }
        // Fence new source admissions; an already selected source publication
        // drains its database operations rather than cancelling uncertain work.
        if let Some(occurrences) = occurrences
            && selection.occurrences.load(Ordering::Acquire)
            && let Err(error) =
                occurrences::publish(database, path, &source_hash, occurrences).await
        {
            status.error = Some(error.to_string());
        }
    }
    selection
        .progress
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .phase = IndexingPhase::Tasks;
    let task_pass = tasks::run(
        &work,
        database,
        root,
        &files,
        stop,
        tasks::Selection {
            selected: selection.documents,
            tracked: selection.tracked,
            today: selection.today,
        },
    )
    .await?;
    status.pending += task_pass.pending;
    if let Some(failure) = task_pass
        .tracked
        .as_ref()
        .and_then(|report| report.failures.first())
    {
        status
            .error
            .get_or_insert_with(|| failure.cause.to_string());
    }
    status.tracked = task_pass.tracked;
    Ok(())
}
