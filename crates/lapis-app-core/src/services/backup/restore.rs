use super::{diagnostics, dto::capture};
use crate::services::{Client, Error, Result};
use lapis_app_protocol::{ErrorCode, repository_restore::*};

impl Client {
    /// Reuse the selected coordinator's accepted-work and writer authority.
    /// Native receipts, not response delivery, determine restore completion.
    pub async fn control_repository_restore(
        &self,
        command: RepositoryRestoreCommand,
    ) -> Result<RepositoryRestoreValue> {
        self.validate_restore(&command)?;
        self.with_backup(move |backup| {
            Box::pin(async move {
                use RepositoryRestoreCommand::*;
                Ok(match command {
                    Preview { checkpoint, path } => {
                        backup.preview_restore(checkpoint, path).await.map(|value| {
                            RepositoryRestoreValue::File(RepositoryFileRestorePreview {
                                checkpoint: value.checkpoint,
                                path: value.path,
                                expected: value.expected,
                                bytes: value.bytes,
                                deleted: value.deleted,
                            })
                        })
                    }
                    PreviewVault { checkpoint } => {
                        backup.preview_vault_restore(checkpoint).await.map(|value| {
                            RepositoryRestoreValue::Vault(RepositoryVaultRestorePreview {
                                checkpoint: value.checkpoint,
                                expected: value.expected,
                                added: value.added,
                                modified: value.modified,
                                deleted: value.deleted,
                            })
                        })
                    }
                    Apply { request } => backup
                        .restore(jj_vault::RestoreRequest {
                            request_id: request.request_id,
                            checkpoint: request.checkpoint,
                            path: request.path,
                            expected: request.expected,
                        })
                        .await
                        .map(capture),
                    ApplyVault { request } => backup
                        .restore_vault(jj_vault::VaultRestoreRequest {
                            request_id: request.request_id,
                            checkpoint: request.checkpoint,
                            expected: request.expected,
                        })
                        .await
                        .map(capture),
                    KeepCurrent {} => backup.capture().await.map(capture),
                })
            })
        })
        .await?
        // The engine still owns uncertain outcomes and exact-retry receipts.
        // Do not parse its human error strings to infer whether a write ran.
        .map_err(diagnostics::restore)
    }

    fn validate_restore(&self, command: &RepositoryRestoreCommand) -> Result<()> {
        use RepositoryRestoreCommand::*;
        let state = self.vault.lock()?;
        state.require_client(self.id)?;
        let (checkpoint, path, request) = match command {
            Preview { checkpoint, path } => (checkpoint, Some(path), None),
            Apply { request } => (
                &request.checkpoint,
                Some(&request.path),
                Some((&request.request_id, &request.expected)),
            ),
            PreviewVault { checkpoint } => (checkpoint, None, None),
            ApplyVault { request } => (
                &request.checkpoint,
                None,
                Some((&request.request_id, &request.expected)),
            ),
            KeepCurrent {} => return Ok(()),
        };
        if checkpoint.is_empty()
            || checkpoint.len() > 256
            || !checkpoint.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(invalid());
        }
        if let Some(path) = path {
            if path.len() > 4096 {
                return Err(invalid());
            }
            state.root.validate_path(path, false)?;
        }
        if let Some((id, expected)) = request
            && (id.is_empty()
                || id.len() > 128
                || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                || !(expected == "missing"
                    || !expected.is_empty()
                        && expected.len() <= 256
                        && expected.bytes().all(|b| b.is_ascii_hexdigit())))
        {
            return Err(invalid());
        }
        Ok(())
    }
}
fn invalid() -> Error {
    Error::new(
        ErrorCode::InvalidRequest,
        "Invalid repository restore request",
    )
}
