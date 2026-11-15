use super::*;
use jj_vault::Backup;
pub(super) fn revision(command: &RepositorySyncCommand) -> u64 {
    match command {
        RepositorySyncCommand::Run { expected_revision }
        | RepositorySyncCommand::ResumeApply {
            expected_revision, ..
        }
        | RepositorySyncCommand::KeepCurrent { expected_revision }
        | RepositorySyncCommand::Inspect {
            expected_revision, ..
        }
        | RepositorySyncCommand::Resolve {
            expected_revision, ..
        } => *expected_revision,
    }
}
pub(super) fn validate(command: &RepositorySyncCommand, saved: &journal::Journal) -> Result<()> {
    match command {
        RepositorySyncCommand::ResumeApply { request_id, .. }
            if saved
                .pending
                .as_ref()
                .is_none_or(|r| r.request_id != *request_id) =>
        {
            Err(stale())
        }
        RepositorySyncCommand::Inspect {
            candidate, path, ..
        }
        | RepositorySyncCommand::Resolve {
            candidate, path, ..
        } => conflict(saved, candidate, path),
        _ => Ok(()),
    }
}
pub(super) async fn apply(
    engine: &Backup,
    selection: &Selection,
    saved: &mut journal::Journal,
    control: &Control,
) -> Result<()> {
    let Some(candidate) = saved.proposal.as_ref().and_then(|p| p.candidate.as_ref()) else {
        return Ok(());
    };
    let preview = engine
        .preview_sync(candidate.commit.clone())
        .await
        .map_err(|_| transfer::failed())?;
    saved.pending = Some(jj_vault::VaultRestoreRequest {
        request_id: uuid::Uuid::new_v4().to_string(),
        checkpoint: preview.checkpoint,
        expected: preview.expected,
    });
    // Durable before physical writes. Never replay this automatically after
    // restart or an error, even when native receipts make exact retry safe.
    saved.save(selection)?;
    control.update(|s| {
        s.phase = RepositorySyncPhase::Applying;
        s.recovery = true;
        s.pending_apply = saved.pending.as_ref().map(|r| r.request_id.clone());
    })?;
    engine
        .apply_sync(saved.pending.clone().ok_or_else(unsupported)?)
        .await
        .map_err(|_| transfer::failed())?;
    saved.pending = None;
    saved.proposal = None;
    saved.save(selection)?;
    control.update(|s| {
        s.recovery = false;
        s.pending_apply = None;
        s.proposal = None;
    })
}
pub(super) async fn run(
    command: RepositorySyncCommand,
    engine: &Backup,
    selection: &Selection,
    saved: &mut journal::Journal,
    control: &Control,
) -> Result<()> {
    match command {
        RepositorySyncCommand::ResumeApply { request_id, .. } => {
            let request = saved
                .pending
                .as_ref()
                .filter(|r| r.request_id == request_id)
                .ok_or_else(stale)?;
            control.update(|s| s.phase = RepositorySyncPhase::Applying)?;
            engine
                .apply_sync(request.clone())
                .await
                .map_err(|_| transfer::failed())?;
            saved.pending = None;
            saved.proposal = None;
            saved.save(selection)?;
        }
        RepositorySyncCommand::KeepCurrent { .. } => {
            engine.capture().await.map_err(|_| transfer::failed())?;
            saved.pending = None;
            saved.proposal = None;
            saved.save(selection)?;
        }
        RepositorySyncCommand::Inspect {
            candidate, path, ..
        } => {
            conflict(saved, &candidate, &path)?;
            let value = engine
                .sync_conflict(candidate, path)
                .await
                .map_err(|_| transfer::failed())?;
            control.update(|s| {
                s.conflict = Some(RepositorySyncConflict {
                    path: value.path,
                    terms: value
                        .terms
                        .into_iter()
                        .map(|t| RepositorySyncTerm {
                            id: t.id,
                            role: t.role,
                            kind: t.kind,
                            text: t.text,
                            executable: t.executable,
                        })
                        .collect(),
                })
            })?;
        }
        RepositorySyncCommand::Resolve {
            candidate,
            path,
            resolution,
            ..
        } => {
            conflict(saved, &candidate, &path)?;
            let resolution = match resolution {
                RepositorySyncResolution::Term { id } => jj_vault::SyncResolution::Term { id },
                RepositorySyncResolution::Text { text, executable } if text.len() <= 1_048_576 => {
                    jj_vault::SyncResolution::Text { text, executable }
                }
                RepositorySyncResolution::Delete => jj_vault::SyncResolution::Delete,
                _ => return Err(stale()),
            };
            let next = engine
                .resolve_sync(candidate, path.clone(), resolution)
                .await
                .map_err(|_| transfer::failed())?;
            let proposal = saved.proposal.as_mut().ok_or_else(stale)?;
            proposal.candidate = Some(next);
            proposal.conflicts.retain(|p| *p != path);
            saved.save(selection)?;
        }
        RepositorySyncCommand::Run { .. } => return Err(unsupported()),
    }
    Ok(())
}
fn conflict(saved: &journal::Journal, candidate: &str, path: &str) -> Result<()> {
    if saved.pending.is_some()
        || !saved.proposal.as_ref().is_some_and(|p| {
            p.candidate.as_ref().is_some_and(|c| c.commit == candidate)
                && p.conflicts.iter().any(|p| p == path)
        })
    {
        return Err(stale());
    }
    Ok(())
}
fn stale() -> Error {
    Error::new(
        ErrorCode::Conflict,
        "Synchronization review changed; refresh before continuing",
    )
}
