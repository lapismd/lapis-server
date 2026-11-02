use super::initialization::{Initialization, Stage};
use crate::services::Result;
use jj_store_jj::vault_workspace::{BackupPath, backup_path};
use jj_vault::Changes;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;

pub(super) fn start(
    root: &Path,
    changes: Changes,
    diagnostics: &Initialization,
) -> Result<RecommendedWatcher> {
    let watched = root.to_owned();
    let mut watcher = diagnostics.watcher(
        Stage::CreateWatcher,
        notify::recommended_watcher(move |event: notify::Result<Event>| {
            let changed = match event {
                Err(_) => true,
                Ok(event) if matches!(event.kind, EventKind::Access(_)) => false,
                Ok(event) => {
                    event.paths.is_empty()
                        || event.paths.iter().any(|path| {
                            path.strip_prefix(&watched).is_ok_and(|relative| {
                                relative.to_str().is_none_or(|path| {
                                    (path.is_empty() || backup_path(path) != BackupPath::Excluded)
                                        && !path.split('/').any(|part| {
                                            matches!(part, ".jj" | ".git" | ".DS_Store")
                                        })
                                })
                            })
                        })
                }
            };
            if changed {
                changes.changed();
            }
        }),
    )?;
    diagnostics.watcher(
        Stage::WatchRoot,
        watcher.watch(root, RecursiveMode::Recursive),
    )?;
    Ok(watcher)
}
