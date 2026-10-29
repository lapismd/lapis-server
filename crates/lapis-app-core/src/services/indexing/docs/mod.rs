mod definition;
mod publication;
mod source;
#[cfg(test)]
mod tests;

use crate::{
    database::Database,
    services::{Error, Result, VaultService},
    vault::VaultRoot,
};
use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, Ordering},
};

impl VaultService {
    /// Native source-only selection after Docs capability admission; no host selects this yet.
    pub async fn start_docs_indexing(self: &Arc<Self>, docs_root: &str) -> Result<()> {
        let root = definition::root(docs_root);
        self.start_markdown_indexing().await?;
        self.database()
            .await?
            .claim_native_projection("docs/note")
            .await;
        let _work = self.begin_work()?;
        if let Some(owner) = self
            .indexing
            .lock()
            .map_err(|_| super::unavailable())?
            .as_ref()
        {
            owner.select_docs(Some(root))?;
        }
        Ok(())
    }
    /// Fence new Docs admissions while accepted database effects drain.
    pub fn stop_docs_indexing(self: &Arc<Self>) -> Result<()> {
        let _work = self.begin_work()?;
        if let Some(owner) = self
            .indexing
            .lock()
            .map_err(|_| super::unavailable())?
            .as_ref()
        {
            owner.select_docs(None)?;
        }
        Ok(())
    }
}

pub(super) async fn run(
    vault: &Weak<VaultService>,
    database: &Database,
    root: &VaultRoot,
    stop: &AtomicBool,
    selection: &Mutex<Option<String>>,
) -> Result<usize> {
    let Some(selected_root) = selection.lock().map_err(|_| super::unavailable())?.clone() else {
        return Ok(0);
    };
    let vault = vault.upgrade().ok_or_else(Error::closed)?;
    let work = vault.begin_work()?;
    let cancelled = || {
        stop.load(Ordering::Acquire)
            || work.cancellation_requested()
            || selection.lock().map_or(true, |value| {
                value.as_deref() != Some(selected_root.as_str())
            })
    };
    if cancelled() {
        return Ok(0);
    }
    let Some(files) = super::manifest::collect(root, cancelled)? else {
        return Ok(0);
    };
    let definition = definition::definition(&selected_root)?;
    if cancelled() {
        return Ok(0);
    }
    definition::register(database, &definition).await?;
    let mut pending = 0usize;
    let mut after = None;
    loop {
        if cancelled() {
            return Ok(pending);
        }
        let paths = database
            .native_projection_source_paths(definition::ID, after.as_deref())
            .await?;
        if paths.is_empty() {
            break;
        }
        after = paths.last().cloned();
        for path in paths {
            if cancelled() {
                return Ok(pending);
            }
            if files.binary_search(&path).is_ok() && definition::matches(&path, &selected_root) {
                continue;
            }
            root.verify_physical_root()?;
            if definition::matches(&path, &selected_root) {
                match root.stat(&path) {
                    Ok(Some(_)) => {
                        pending += 1;
                        continue;
                    }
                    Ok(None) => {}
                    Err(error)
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::NotFound
                        ) => {}
                    Err(error) => return Err(error.into()),
                }
            }
            database
                .delete_projection_source(definition::ID, &path, Some("docs"))
                .await?;
        }
    }
    let mut failure = None;
    for path in files
        .iter()
        .filter(|path| definition::matches(path, &selected_root))
    {
        if cancelled() {
            return Ok(pending);
        }
        match source::publish(database, root, &definition, path, cancelled).await {
            Ok(count) => pending = pending.saturating_add(count),
            Err(error) => {
                failure.get_or_insert(error);
            }
        }
    }
    failure.map_or(Ok(pending), Err)
}
