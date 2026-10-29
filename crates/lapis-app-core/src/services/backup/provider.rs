use super::super::VaultService;
use jj_vault::{
    AdoptionRequest, AiHistoryRequest, AiHistoryResponse, Capture, Checkpoint, FetchedHead,
    NativeRepository, Repository, RestorePreview, RestoreRequest, SyncConflict, SyncProposal,
    SyncResolution, VaultRestorePreview, VaultRestoreRequest,
};
use lapis_app_protocol::VaultPhase;
use std::sync::Weak;

/// The engine runs on its own native worker, under the same physical-root/file
/// gate as service writes. Notification callbacks never acquire that gate.
pub(super) struct Provider {
    pub native: NativeRepository,
    pub vault: Weak<VaultService>,
}
impl Repository for Provider {
    fn recover_ai_history(&mut self) -> std::result::Result<(), String> {
        self.with_native(true, |native| native.recover_ai_history())
    }
    fn capture(&mut self) -> std::result::Result<Capture, String> {
        self.with_native(false, |native| native.capture())
    }
    fn ai_history(
        &mut self,
        request: AiHistoryRequest,
    ) -> std::result::Result<AiHistoryResponse, String> {
        self.with_native(matches!(request, AiHistoryRequest::Undo(_)), |native| {
            native.ai_history(request)
        })
    }
    fn adopt_history(
        &mut self,
        request: &AdoptionRequest,
    ) -> std::result::Result<Checkpoint, String> {
        self.with_native(false, |native| native.adopt_history(request))
    }
    fn stop_adoption(&mut self, id: &str) -> std::result::Result<(), String> {
        self.with_native(false, |native| native.stop_adoption(id))
    }
    fn prepare_sync(&mut self, incoming: &str) -> std::result::Result<SyncProposal, String> {
        self.with_native(false, |native| native.prepare_sync(incoming))
    }
    fn preview_sync(
        &mut self,
        candidate: &str,
    ) -> std::result::Result<VaultRestorePreview, String> {
        self.with_native(false, |native| native.preview_sync(candidate))
    }
    fn apply_sync(
        &mut self,
        request: &VaultRestoreRequest,
    ) -> std::result::Result<Capture, String> {
        self.with_native(true, |native| native.apply_sync(request))
    }
    fn sync_conflict(
        &mut self,
        candidate: &str,
        path: &str,
    ) -> std::result::Result<SyncConflict, String> {
        self.with_native(false, |native| native.sync_conflict(candidate, path))
    }
    fn resolve_sync(
        &mut self,
        candidate: &str,
        path: &str,
        resolution: SyncResolution,
    ) -> std::result::Result<Checkpoint, String> {
        self.with_native(false, |native| {
            native.resolve_sync(candidate, path, resolution)
        })
    }
    fn import_remote(&mut self, head: &FetchedHead) -> std::result::Result<Checkpoint, String> {
        self.with_native(false, |native| native.import_remote(head))
    }
    fn restore_remote(
        &mut self,
        request: &VaultRestoreRequest,
    ) -> std::result::Result<Capture, String> {
        self.with_native(true, |native| native.restore_remote(request))
    }
    fn preview_vault_restore(
        &mut self,
        checkpoint: &str,
    ) -> std::result::Result<VaultRestorePreview, String> {
        self.with_native(false, |native| native.preview_vault_restore(checkpoint))
    }
    fn restore_vault(
        &mut self,
        request: &VaultRestoreRequest,
    ) -> std::result::Result<Capture, String> {
        self.with_native(true, |native| native.restore_vault(request))
    }
    fn preview_restore(
        &mut self,
        checkpoint: &str,
        path: &str,
    ) -> std::result::Result<RestorePreview, String> {
        self.with_native(false, |native| native.preview_restore(checkpoint, path))
    }
    fn restore(&mut self, request: &RestoreRequest) -> std::result::Result<Capture, String> {
        self.with_native(true, |native| native.restore(request))
    }
}
impl Provider {
    fn with_native<T>(
        &mut self,
        writes: bool,
        action: impl FnOnce(&mut NativeRepository) -> std::result::Result<T, String>,
    ) -> std::result::Result<T, String> {
        let vault = self.vault.upgrade().ok_or("Vault service is closed")?;
        let storage = vault
            .backup_storage
            .as_ref()
            .ok_or("Native history storage is not selected")?;
        // Match host ordering: external file gate, then the shared vault gate.
        let _files = storage
            .files
            .as_ref()
            .map(|files| files.lock())
            .transpose()
            .map_err(|_| "Vault file IO is unavailable")?;
        let state = vault.lock().map_err(|error| error.to_string())?;
        if state.phase == VaultPhase::Closed {
            return Err("Vault service is closed".into());
        }
        if writes && state.editors_blocked(None) {
            return Err("Vault has unresolved editor state".into());
        }
        state
            .root
            .verify_physical_root()
            .map_err(|error| error.to_string())?;
        vault
            .backup_storage
            .as_ref()
            .ok_or("Native history storage is not selected")?
            .validate_vault(&state.root)
            .map_err(|error| error.to_string())?;
        self.native
            .binding
            .relink()
            .map_err(|error| error.to_string())?;
        action(&mut self.native)
    }
}
