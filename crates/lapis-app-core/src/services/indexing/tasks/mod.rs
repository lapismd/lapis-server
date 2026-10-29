mod definition;
mod publication;
mod snapshot;
mod tracked;
pub use tracked::{TrackedTaskError, TrackedTaskFailure, TrackedTaskProvider, TrackedTaskStatus};
#[cfg(test)]
mod tests;

use crate::{
    database::Database,
    services::{Result, Work},
    vault::VaultRoot,
};
use std::{
    collections::HashSet,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

pub(super) struct Selection<'a> {
    pub selected: &'a AtomicBool,
    pub tracked: &'a Mutex<Option<TrackedTaskProvider>>,
    pub today: &'a str,
}
#[derive(Default)]
pub(super) struct Pass {
    pub pending: usize,
    pub tracked: Option<TrackedTaskStatus>,
}

pub(super) async fn run(
    work: &Work,
    database: &Database,
    root: &VaultRoot,
    files: &[String],
    stop: &AtomicBool,
    selection: Selection<'_>,
) -> Result<Pass> {
    let selected = selection.selected;
    if stop.load(Ordering::Acquire) || !selected.load(Ordering::Acquire) {
        return Ok(Pass::default());
    }
    definition::register(database).await?;
    let snapshot = match snapshot::Snapshot::prepare(root, files, stop) {
        Ok(snapshot) => snapshot,
        Err(error) if error.code() == lapis_app_protocol::ErrorCode::NotFound => {
            return Ok(Pass {
                pending: 1,
                ..Default::default()
            });
        }
        Err(error) => return Err(error),
    };
    if !snapshot.unchanged(root, files, stop)? {
        return Ok(Pass {
            pending: usize::from(!stop.load(Ordering::Acquire)),
            ..Default::default()
        });
    }
    let paths: HashSet<_> = snapshot
        .documents
        .iter()
        .map(|doc| doc.path.as_str())
        .collect();
    for path in files.iter().filter(|path| super::records::markdown(path)) {
        if stop.load(Ordering::Acquire) || !selected.load(Ordering::Acquire) {
            return Ok(Pass::default());
        }
        if !paths.contains(path.as_str()) {
            publication::prune(database, path).await?;
        }
    }
    for doc in &snapshot.documents {
        if stop.load(Ordering::Acquire) || !selected.load(Ordering::Acquire) {
            return Ok(Pass::default());
        }
        root.verify_physical_root()?;
        match root.read_text(&doc.path) {
            Ok(current) if current == doc.raw => {}
            Ok(_) => {
                return Ok(Pass {
                    pending: 1,
                    ..Default::default()
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Pass {
                    pending: 1,
                    ..Default::default()
                });
            }
            Err(error) => return Err(error.into()),
        }
        publication::publish(
            database,
            doc,
            snapshot.structures.get(&doc.path),
            selection.today,
        )
        .await?;
    }
    let tracked = tracked::run(
        work,
        database,
        &snapshot.documents,
        stop,
        selected,
        selection.tracked,
        selection.today,
    )
    .await?;
    Ok(Pass {
        pending: tracked.as_ref().map_or(0, |status| status.changed),
        tracked,
    })
}
