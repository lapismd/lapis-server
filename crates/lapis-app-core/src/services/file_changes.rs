//! One bounded native watcher per admitted vault; subscribers own no work lease.
use super::{Client, Error, Result};
use crate::vault::VaultRoot;
use lapis_app_protocol::{
    ErrorCode,
    file_changes::{FileChange, FileChangeSet},
};
use notify::{
    Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher,
    event::{ModifyKind, RenameMode},
};
use std::{
    collections::VecDeque,
    path::{Component, Path},
    sync::{Arc, Mutex},
};
use tokio::sync::broadcast;

const HISTORY_BYTES: usize = 2 * 1024 * 1024;
const HISTORY_FRAMES: usize = 256;

pub struct FileObservation {
    pub ready: FileChangeSet,
    pub pending: VecDeque<FileChangeSet>,
    pub changes: broadcast::Receiver<FileChangeSet>,
}
struct Journal {
    revision: u64,
    frames: VecDeque<(FileChangeSet, usize)>,
    bytes: usize,
    sender: broadcast::Sender<FileChangeSet>,
}
pub(super) struct Owner {
    journal: Arc<Mutex<Journal>>,
    _watcher: RecommendedWatcher,
}
impl Journal {
    fn publish(&mut self, changes: Vec<FileChange>, reset: bool) {
        self.revision += 1;
        let mut frame = FileChangeSet {
            revision: self.revision,
            reset,
            changes,
        };
        let mut size = serde_json::to_vec(&frame).map_or(HISTORY_BYTES, |v| v.len());
        if size > 120 * 1024 {
            frame.changes.clear();
            frame.reset = true;
            size = serde_json::to_vec(&frame).map_or(128, |v| v.len());
        }
        self.bytes += size;
        self.frames.push_back((frame.clone(), size));
        while self.frames.len() > HISTORY_FRAMES || self.bytes > HISTORY_BYTES {
            if let Some((_, size)) = self.frames.pop_front() {
                self.bytes -= size;
            }
        }
        let _ = self.sender.send(frame);
    }
    fn subscribe(&self, cursor: Option<u64>) -> FileObservation {
        let complete = cursor.is_some_and(|value| {
            value <= self.revision
                && self
                    .frames
                    .front()
                    .is_none_or(|(first, _)| value >= first.revision.saturating_sub(1))
        });
        FileObservation {
            ready: FileChangeSet {
                revision: if complete {
                    cursor.unwrap()
                } else {
                    self.revision
                },
                reset: cursor.is_some() && !complete,
                changes: vec![],
            },
            pending: if complete {
                self.frames
                    .iter()
                    .filter(|(f, _)| f.revision > cursor.unwrap())
                    .map(|(f, _)| f.clone())
                    .collect()
            } else {
                VecDeque::new()
            },
            changes: self.sender.subscribe(),
        }
    }
}
fn relative(root: &VaultRoot, path: &Path) -> Option<String> {
    let path = path.strip_prefix(root.path()).ok()?;
    if !path.components().all(|p| matches!(p, Component::Normal(_))) {
        return None;
    }
    let value = path.to_str()?.replace(std::path::MAIN_SEPARATOR, "/");
    // App state has a dedicated configuration-file observer; caches and plugin
    // payloads never become user-file notifications. No file bytes are read.
    if value == ".lapis" || (value.starts_with(".lapis/") && value != ".lapis/app.json") {
        return None;
    }
    root.validate_path(&value, false).ok()?;
    Some(value)
}
fn translate(root: &VaultRoot, event: Event) -> Option<(Vec<FileChange>, bool)> {
    if matches!(event.kind, EventKind::Access(_)) {
        return None;
    }
    if event.need_rescan() || matches!(event.kind, EventKind::Other | EventKind::Any) {
        return Some((vec![], true));
    }
    if event.paths.len() > 64 {
        return Some((vec![], true));
    }
    let changes = if matches!(
        event.kind,
        EventKind::Modify(ModifyKind::Name(RenameMode::Both))
    ) && event.paths.len() == 2
    {
        match (
            relative(root, &event.paths[0]),
            relative(root, &event.paths[1]),
        ) {
            (Some(previous_path), Some(path)) => vec![FileChange::Rename {
                path,
                previous_path,
            }],
            (Some(path), None) => vec![FileChange::Delete { path }],
            (None, Some(path)) => vec![FileChange::Create { path }],
            _ => vec![],
        }
    } else {
        event
            .paths
            .iter()
            .filter_map(|p| relative(root, p))
            .map(|path| match event.kind {
                EventKind::Create(_) => FileChange::Create { path },
                EventKind::Remove(_) => FileChange::Delete { path },
                _ => FileChange::Modify { path },
            })
            .collect()
    };
    (!changes.is_empty()).then_some((changes, false))
}
impl Owner {
    fn start(root: VaultRoot) -> Result<Self> {
        root.verify_physical_root()?;
        let path = root.path().to_owned();
        let journal = Arc::new(Mutex::new(Journal {
            revision: 0,
            frames: VecDeque::new(),
            bytes: 0,
            sender: broadcast::channel(HISTORY_FRAMES).0,
        }));
        let sink = journal.clone();
        let mut watcher = RecommendedWatcher::new(
            move |event: notify::Result<Event>| {
                let update = match event {
                    Ok(event) => translate(&root, event),
                    Err(_) => Some((vec![], true)),
                };
                if let Some((changes, reset)) = update
                    && let Ok(mut journal) = sink.lock()
                {
                    journal.publish(changes, reset);
                }
            },
            notify::Config::default().with_follow_symlinks(false),
        )
        .map_err(|_| Error::new(ErrorCode::Io, "File observation is unavailable"))?;
        watcher
            .watch(&path, RecursiveMode::Recursive)
            .map_err(|_| Error::new(ErrorCode::Io, "File observation is unavailable"))?;
        Ok(Self {
            journal,
            _watcher: watcher,
        })
    }
}
impl Client {
    pub fn validate_file_observation(&self) -> Result<()> {
        let state = self.vault.lock()?;
        state.require_client(self.id)?;
        state.root.verify_physical_root()?;
        Ok(())
    }
    pub fn subscribe_to_file_changes(&self, cursor: Option<u64>) -> Result<FileObservation> {
        let state = self.vault.lock()?;
        state.require_client(self.id)?;
        state.root.verify_physical_root()?;
        let mut owner = self
            .vault
            .file_changes
            .lock()
            .map_err(|_| Error::closed())?;
        if owner.is_none() {
            *owner = Some(Owner::start(state.root.try_clone()?)?);
        }
        let journal = owner
            .as_ref()
            .unwrap()
            .journal
            .lock()
            .map_err(|_| Error::closed())?;
        Ok(journal.subscribe(cursor))
    }
}
#[cfg(test)]
mod tests;
