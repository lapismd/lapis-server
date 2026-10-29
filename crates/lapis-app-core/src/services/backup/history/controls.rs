//! Admitted reads borrow the existing immutable source, never the capture lease.
use super::{not_selected, requests, unavailable};
use crate::services::{Client, Error, Result, VaultService};
use jj_vault::search::{NativeHistory, indexed};
use lapis_app_protocol::{ErrorCode, history_index::*};
use serde::{Serialize, de::DeserializeOwned};
use std::sync::Arc;

impl Client {
    /// Accepted work drains with its vault even if its HTTP/UI reply disappears.
    pub async fn control_history_index(
        &self,
        command: HistoryIndexCommand,
    ) -> Result<HistoryIndexValue> {
        self.vault.lock()?.require_client(self.id)?;
        requests::validate(&command)?;
        let work = self.vault.begin_work()?;
        let vault = self.vault.clone();
        tokio::task::spawn_blocking(move || {
            let _work = work;
            vault.history_index(command)
        })
        .await
        .map_err(|_| unavailable())?
    }
}

impl VaultService {
    fn history_index(self: &Arc<Self>, command: HistoryIndexCommand) -> Result<HistoryIndexValue> {
        // Serialize only engine access and its source fences. Start/status may
        // visit the worker slot, whose retirement joins the active batch.
        let _access = if matches!(
            command,
            HistoryIndexCommand::Query { .. } | HistoryIndexCommand::Context { .. }
        ) {
            Some(
                self.history_index_access
                    .lock()
                    .map_err(|_| unavailable())?,
            )
        } else {
            None
        };
        let storage = self
            .backup_storage
            .as_ref()
            .filter(|storage| storage.history_indexing)
            .ok_or_else(not_selected)?;
        let source = storage.history_source(self)?;
        let missing = match std::fs::symlink_metadata(source.binding.state.join("vault.json")) {
            Ok(_) => false,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => true,
            Err(_) => return Err(unavailable()),
        };
        let value = if missing {
            empty(command)?
        } else {
            match command {
                HistoryIndexCommand::Start {} | HistoryIndexCommand::Status {} => {
                    let status = if matches!(command, HistoryIndexCommand::Start {}) {
                        self.start_history_indexing()?
                    } else {
                        self.history_indexing_status()?
                    };
                    if status.error.is_some() {
                        return Err(unavailable());
                    }
                    HistoryIndexValue::Status(project(status.source.unwrap_or(indexed::Status {
                        generation: 0,
                        indexed: 0,
                        omitted: 0,
                        complete: false,
                    }))?)
                }
                HistoryIndexCommand::Query { search } => HistoryIndexValue::Page(project(
                    source
                        .indexed_search(&project(search)?)
                        .map_err(|_| unavailable())?,
                )?),
                HistoryIndexCommand::Context {
                    commit,
                    path,
                    start,
                    end,
                } => HistoryIndexValue::Context(context(&source, &commit, &path, start, end)?),
            }
        };
        // Resolving again fences replaced vault/profile roots and storage moves.
        let current = storage.history_source(self)?;
        if current.binding.state != source.binding.state {
            return Err(Error::busy());
        }
        Ok(value)
    }
}

fn project<T: DeserializeOwned>(value: impl Serialize) -> Result<T> {
    serde_json::to_value(value)
        .and_then(serde_json::from_value)
        .map_err(|_| unavailable())
}

fn empty(command: HistoryIndexCommand) -> Result<HistoryIndexValue> {
    let status = HistoryIndexProgress {
        generation: 0,
        indexed: 0,
        omitted: 0,
        complete: true,
    };
    Ok(match command {
        HistoryIndexCommand::Start {} | HistoryIndexCommand::Status {} => {
            HistoryIndexValue::Status(status)
        }
        HistoryIndexCommand::Query { .. } => HistoryIndexValue::Page(HistoryIndexPage {
            status,
            hits: vec![],
            next: None,
            reset: false,
        }),
        HistoryIndexCommand::Context { .. } => {
            return Err(Error::new(
                ErrorCode::NotFound,
                "This vault has no saved history",
            ));
        }
    })
}

fn context(
    source: &NativeHistory,
    commit: &str,
    path: &str,
    start: usize,
    end: usize,
) -> Result<HistoryIndexContext> {
    let text = source
        .indexed_text(commit, path)
        .map_err(|_| unavailable())?
        .ok_or_else(|| Error::new(ErrorCode::NotFound, "Historical text is unavailable"))?;
    let lines = text.lines().collect::<Vec<_>>();
    let selected = lines
        .get(start..end.min(lines.len()))
        .ok_or_else(|| {
            Error::new(
                ErrorCode::InvalidRequest,
                "Historical context is unavailable",
            )
        })?
        .join("\n");
    if selected.len() > 32 * 1024 {
        return Err(Error::new(
            ErrorCode::InvalidRequest,
            "Historical context exceeds its read budget",
        ));
    }
    Ok(HistoryIndexContext {
        text: selected,
        start,
        end: end.min(lines.len()),
        before: start > 0,
        after: end < lines.len(),
    })
}
