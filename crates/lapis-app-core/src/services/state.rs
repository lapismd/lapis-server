use super::{DatabaseStorage, Error, Result};
use crate::database::Database;
use crate::vault::VaultRoot;
use lapis_app_protocol::{ErrorCode, VaultPhase, VaultStatus};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Condvar, Mutex, MutexGuard},
};
use uuid::Uuid;

pub struct VaultService {
    pub(super) skills: Mutex<super::agent_skills::State>,
    pub(super) id: Uuid,
    pub(super) generation: Uuid,
    pub(super) state: Mutex<State>,
    pub(super) changed: Condvar,
    pub(super) storage: Option<Arc<DatabaseStorage>>,
    pub(super) database_open: tokio::sync::Mutex<()>,
    pub(super) indexing: Mutex<Option<super::indexing::Owner>>,
    pub(super) indexing_control: tokio::sync::Mutex<()>,
    pub(super) backup_storage: Option<Arc<super::BackupStorage>>,
    pub(super) backup: super::backup::Slot,
    pub(super) history_indexing: Mutex<Option<Arc<super::backup::history::Owner>>>,
    pub(super) history_index_access: Mutex<()>,
    #[cfg(test)]
    pub(super) history_index_pause: Mutex<Option<Box<dyn FnOnce() + Send>>>,
    pub(super) history_reader: Mutex<Option<Arc<super::history::Owner>>>,
    pub(super) close_gate: Mutex<()>,
    pub(super) task_lifecycle: Mutex<()>,
    pub(super) task_storage: Option<Arc<super::TaskStorage>>,
    pub(super) tasks: Mutex<Option<Arc<super::tasks::service::Owner>>>,
    pub(super) turn_storage: Option<Arc<super::TurnStorage>>,
    pub(super) turns: Mutex<Option<Arc<super::turns::Owner>>>,
}
pub(super) struct Editor {
    pub client: Uuid,
    pub path: String,
    pub dirty: bool,
    pub revision: u32,
    pub detached: bool,
    pub intent: Option<EditingIntent>,
}
pub(super) struct EditingIntent {
    pub generation: u32,
    pub active: bool,
}
impl Editor {
    pub fn blocked(&self) -> bool {
        self.dirty || self.intent.as_ref().is_some_and(|intent| intent.active)
    }
}
pub(super) struct State {
    pub root: VaultRoot,
    pub phase: VaultPhase,
    pub clients: HashSet<Uuid>,
    pub work: usize,
    pub editors: HashMap<Uuid, Editor>,
    pub retired_editor_intents: HashMap<Uuid, Uuid>,
    pub database: Option<Database>,
    pub database_starting: bool,
    pub database_error: Option<lapis_app_protocol::ServiceError>,
    pub backup_pending: bool,
}
impl State {
    pub fn require_open(&self) -> Result<()> {
        if self.phase != VaultPhase::Open {
            return Err(Error::closed());
        }
        Ok(())
    }
    pub fn require_client(&self, id: Uuid) -> Result<()> {
        self.require_open()?;
        if !self.clients.contains(&id) {
            return Err(Error::denied());
        }
        Ok(())
    }
    pub fn editors_blocked(&self, path: Option<&str>) -> bool {
        self.editors
            .values()
            .any(|e| e.blocked() && path.is_none_or(|path| e.path == path))
    }
    pub fn begin_close(&mut self) {
        if let Some(database) = &self.database {
            database.fence();
        }
        self.phase = VaultPhase::Closing;
        self.advance_close();
    }
    pub fn advance_close(&mut self) {
        if self.phase == VaultPhase::Closing
            && self.work == 0
            && self.database.is_none()
            && !self.database_starting
            && !self.backup_pending
        {
            self.phase = VaultPhase::Closed;
        }
    }
}
impl VaultService {
    pub(super) fn new(id: Uuid, root: VaultRoot, storage: Option<Arc<DatabaseStorage>>) -> Self {
        Self {
            skills: Mutex::new(super::agent_skills::State::default()),
            id,
            generation: Uuid::new_v4(),
            state: Mutex::new(State {
                root,
                phase: VaultPhase::Open,
                clients: HashSet::new(),
                work: 0,
                editors: HashMap::new(),
                retired_editor_intents: HashMap::new(),
                database: None,
                database_starting: false,
                database_error: None,
                backup_pending: false,
            }),
            changed: Condvar::new(),
            storage,
            database_open: tokio::sync::Mutex::new(()),
            indexing: Mutex::new(None),
            indexing_control: tokio::sync::Mutex::new(()),
            backup_storage: None,
            backup: super::backup::Slot::default(),
            history_indexing: Mutex::new(None),
            history_index_access: Mutex::new(()),
            #[cfg(test)]
            history_index_pause: Mutex::new(None),
            history_reader: Mutex::new(None),
            close_gate: Mutex::new(()),
            task_lifecycle: Mutex::new(()),
            task_storage: None,
            tasks: Mutex::new(None),
            turn_storage: None,
            turns: Mutex::new(None),
        }
    }
    pub fn id(&self) -> Uuid {
        self.id
    }
    pub(super) fn lock(&self) -> Result<MutexGuard<'_, State>> {
        self.state
            .lock()
            .map_err(|_| Error::new(ErrorCode::Io, "Vault state is unavailable"))
    }
    pub fn status(&self) -> Result<VaultStatus> {
        let state = self.lock()?;
        Ok(VaultStatus {
            vault_id: self.id,
            generation: self.generation,
            phase: state.phase,
            clients: state.clients.len(),
            work: state.work,
            dirty_editors: state.editors.values().filter(|e| e.dirty).count(),
        })
    }
    /// Blocking drain; async hosts call this outside their worker executor.
    pub fn close(&self) -> Result<()> {
        let _close = self.close_gate.lock().map_err(|_| Error::closed())?;
        if self.turns_failed() {
            return Err(Error::busy());
        }
        if self.backup_storage.is_some() {
            self.finish_backup()?;
        } else {
            let mut state = self.lock()?;
            if state.editors_blocked(None) {
                return Err(Error::busy());
            }
            state.begin_close();
        }
        self.finish_history_reader()?;
        self.finish_history_indexing()?;
        self.finish_indexing()?;
        let mut state = self.lock()?;
        if state.editors_blocked(None) {
            return Err(Error::busy());
        }
        state.begin_close();
        while state.phase != VaultPhase::Closed {
            if self.turns_failed() {
                return Err(Error::busy());
            }
            state = self
                .changed
                .wait(state)
                .map_err(|_| Error::new(ErrorCode::Io, "Vault drain is unavailable"))?;
        }
        let database_error = state.database_error.clone();
        drop(state);
        self.retire_turns()?;
        self.retire_tasks()?;
        match database_error {
            Some(error) => Err(Error(error)),
            None => Ok(()),
        }
    }
}
