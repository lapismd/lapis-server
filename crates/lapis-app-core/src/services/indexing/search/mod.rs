mod canvas;
mod checksum;
mod configuration;
pub use configuration::SearchIndexingConfiguration;
pub(super) use configuration::{Policy, Selection};
mod controls;
mod envelope;
mod manifest;
mod source;
#[cfg(test)]
mod tests;

use crate::{
    database::Database,
    services::{Error, Result, VaultService},
    vault::VaultRoot,
};
use lapis_app_protocol::database::indexed::queries::ManifestQuery;
use lapis_plugin_services::markdown::PARSER_VERSION;
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, Ordering},
};

impl VaultService {
    /// Select the native Markdown Search source; other providers and host defaults stay separate.
    pub async fn start_search_indexing(self: &Arc<Self>) -> Result<()> {
        self.start_search_indexing_with(SearchIndexingConfiguration::default())
            .await
    }
    /// Select built-in sources and validated chunking on the existing indexing worker.
    pub async fn start_search_indexing_with(
        self: &Arc<Self>,
        configuration: SearchIndexingConfiguration,
    ) -> Result<()> {
        configuration.validate()?;
        let _control = self.indexing_control.lock().await;
        self.start_search_policy(configuration.into()).await
    }
    async fn start_search_policy(self: &Arc<Self>, configuration: Policy) -> Result<()> {
        self.database().await?.claim_native_search().await;
        self.start_markdown_indexing().await?;
        let _work = self.begin_work()?;
        let slot = self.indexing.lock().map_err(|_| super::unavailable())?;
        if let Some(owner) = slot.as_ref() {
            owner.select_search(Some(configuration))?;
        }
        Ok(())
    }
    /// Fence new Search source admissions; accepted database work retains its drain contract.
    pub fn stop_search_indexing(self: &Arc<Self>) -> Result<()> {
        let _work = self.begin_work()?;
        let slot = self.indexing.lock().map_err(|_| super::unavailable())?;
        if let Some(owner) = slot.as_ref() {
            owner.select_search(None)?;
        }
        Ok(())
    }
}

pub(super) async fn run(
    vault: &Weak<VaultService>,
    database: &Database,
    root: &VaultRoot,
    stop: &AtomicBool,
    selected: &Selection,
    progress: &Mutex<super::IndexingStatus>,
    rebuild: bool,
) -> Result<usize> {
    let Some((policy, generation)) = selected.snapshot()? else {
        return Ok(0);
    };
    if stop.load(Ordering::Acquire) {
        return Ok(0);
    }
    let vault = vault.upgrade().ok_or_else(Error::closed)?;
    let work = vault.begin_work()?;
    let cancelled = || {
        stop.load(Ordering::Acquire)
            || !selected.current(generation)
            || work.cancellation_requested()
    };
    let Some(files) = super::manifest::collect(root, cancelled)? else {
        return Ok(0);
    };
    let Some(mut search) = manifest::collect(database, cancelled).await? else {
        return Ok(0);
    };
    {
        let mut status = progress.lock().unwrap_or_else(|e| e.into_inner());
        status.processed = 0;
        status.total = files
            .iter()
            .filter(|path| {
                policy.markdown && super::records::markdown(path)
                    || policy.options.canvas && canvas::accepts(path)
            })
            .count();
    }
    let mut after = None;
    let mut pending = 0usize;
    let mut failure = None;
    let configuration = &policy.options;
    if policy.markdown {
        loop {
            if cancelled() {
                return Ok(pending);
            }
            let page = database
                .list_indexed_file_manifest(ManifestQuery {
                    after,
                    limit: Some(500),
                    ..Default::default()
                })
                .await?;
            for file in page.rows {
                if cancelled() {
                    return Ok(pending);
                }
                if !super::records::markdown(&file.path) || files.binary_search(&file.path).is_err()
                {
                    continue;
                }
                progress.lock().unwrap_or_else(|e| e.into_inner()).processed += 1;
                let existing = search.remove(&file.path);
                if existing.as_ref().is_some_and(|row| {
                    row.source_provider_id.as_deref() != Some(envelope::PROVIDER)
                }) {
                    continue;
                }
                if file.parser_version.as_deref() != Some(PARSER_VERSION) {
                    continue;
                }
                if existing
                    .as_ref()
                    .is_some_and(|row| source::unchanged(&file, row, &configuration.chunking))
                {
                    continue;
                }
                match source::publish(database, root, &file, &configuration.chunking, cancelled)
                    .await
                {
                    Ok(count) => pending = pending.saturating_add(count),
                    Err(error) => {
                        failure.get_or_insert(error);
                    }
                }
            }
            after = page.next_cursor;
            if after.is_none() {
                break;
            }
        }
    }
    if configuration.canvas {
        match canvas::run(
            database,
            root,
            &files,
            &mut search,
            &configuration.chunking,
            progress,
            cancelled,
        )
        .await
        {
            Ok(count) => pending = pending.saturating_add(count),
            Err(error) => {
                failure.get_or_insert(error);
            }
        }
    }
    for row in search.into_values().filter(|row| {
        row.source_provider_id.as_deref() == Some(envelope::PROVIDER)
            || (configuration.canvas || policy.prune_unselected)
                && row.source_provider_id.as_deref() == Some(canvas::PROVIDER)
    }) {
        if cancelled() {
            return Ok(pending);
        }
        let supported = policy.markdown && super::records::markdown(&row.path)
            || configuration.canvas && canvas::accepts(&row.path);
        if files.binary_search(&row.path).is_ok() && supported {
            continue;
        }
        root.verify_physical_root()?;
        // A visible file created after enumeration must wait for the next pass.
        let hidden = row.path.split('/').any(|part| part.starts_with('.'));
        if !hidden && supported && root.stat(&row.path)?.is_some() {
            pending = pending.saturating_add(1);
            continue;
        }
        database.delete_search_document(&row.path).await?;
    }
    if rebuild && failure.is_none() && !cancelled() {
        database.rebuild_search_index().await?;
    }
    failure.map_or(Ok(pending), Err)
}
