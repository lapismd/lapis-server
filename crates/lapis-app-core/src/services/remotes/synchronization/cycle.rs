use super::*;
use jj_store_jj::vault_workspace::HistoryReader;
use jj_vault::{Backup, PushOutcome};

pub(super) fn execute(
    vault: &Arc<VaultService>,
    command: Option<RepositorySyncCommand>,
) -> Result<()> {
    let control = &vault.synchronization.control;
    let result = (|| {
        let storage = vault.remotes_storage.as_ref().ok_or_else(unsupported)?;
        let _gate = storage.gate.lock().map_err(|_| Error::busy())?;
        let _process = custody::lock(storage)?;
        let selection = storage.selection(vault)?;
        selection.observe_credentials(custody::epoch(storage)?)?;
        let current = selection.read()?;
        let (_guard, state) = selection.guard(current.revision)?;
        vault.synchronization.selection(&state);
        if let Some(command) = &command {
            state.expect(recovery::revision(command))?;
        }
        if state.backup.is_none() {
            return if command.is_some() {
                Err(unsupported())
            } else {
                Ok(())
            };
        }
        let mut saved = journal::Journal::open(&selection, &state, &backup::source(vault)?)?;
        observation::project(control, &saved, &state)?;
        if let Some(command) = &command {
            recovery::validate(command, &saved)?;
            if matches!(command, RepositorySyncCommand::Run { .. }) && !state.automatic_sync {
                return Err(Error::new(
                    ErrorCode::PermissionDenied,
                    "Enable automatic synchronization before synchronizing",
                ));
            }
        }
        if command.is_none()
            && (!state.automatic_sync
                || saved.pending.is_some()
                || saved
                    .proposal
                    .as_ref()
                    .is_some_and(|p| !p.conflicts.is_empty()))
        {
            return Ok(());
        }
        control.update(|s| {
            s.busy = true;
            s.conflict = None;
        })?;
        // Engine creation owns its own executor and must precede block_on.
        let owner = crate::services::backup::remote::synchronization_owner(vault)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        runtime.block_on(async {
            let engine = owner.engine.lock().await;
            if let Some(command) = command {
                if !matches!(command, RepositorySyncCommand::Run { .. }) {
                    recovery::run(command, &engine, &selection, &mut saved, control).await?;
                    observation::project(control, &saved, &state)?;
                    return Ok(());
                }
                if !state.automatic_sync {
                    return Err(Error::new(
                        ErrorCode::PermissionDenied,
                        "Enable automatic synchronization before synchronizing",
                    ));
                }
            }
            if saved.pending.is_some() {
                return Err(Error::new(
                    ErrorCode::Conflict,
                    "Retry the original application or keep current files before synchronizing",
                ));
            }
            synchronize(
                vault, storage, &selection, &state, &engine, &mut saved, control,
            )
            .await?;
            observation::project(control, &saved, &state)
        })
    })();
    let _ = control.update(|s| {
        s.busy = false;
        if result.is_err() {
            s.phase = RepositorySyncPhase::Attention;
        }
    });
    result
}

async fn synchronize(
    vault: &Arc<VaultService>,
    storage: &RemoteStorage,
    selection: &Selection,
    state: &SelectionState,
    engine: &Backup,
    saved: &mut journal::Journal,
    control: &Control,
) -> Result<()> {
    let connection = state.backup.as_ref().ok_or_else(unsupported)?;
    operations::require_push(connection)?;
    let binding = backup::binding(vault)?;
    let transfer = transfer::connect(storage, binding.clone(), connection).await?;
    let mut publication = backup_journal::Journal::open(selection, state, &backup::source(vault)?)?;
    if let Some(pending) = operations::pending(&binding)? {
        // Observation alone reconciles the retained request; this cycle cannot
        // publish a new intent after an uncertain earlier outcome.
        control.update(|s| s.phase = RepositorySyncPhase::Publishing)?;
        let outcome = transfer
            .reconcile(&pending.request_id)
            .await
            .map_err(|_| transfer::failed())?;
        if matches!(
            outcome,
            PushOutcome::Complete | PushOutcome::Stale | PushOutcome::Stopped
        ) {
            publication.review = None;
            publication.save(selection)?;
        } else {
            return Err(transfer::failed());
        }
        return Ok(());
    }
    if publication.review.is_some() {
        return Err(Error::new(
            ErrorCode::Busy,
            "An explicit backup push review is still pending",
        ));
    }
    control.update(|s| s.phase = RepositorySyncPhase::Fetching)?;
    let head = transfer.fetch().await.map_err(|_| transfer::failed())?;
    let remote = head.as_ref().map(|h| h.commit().to_owned());
    if let Some(head) = head {
        engine
            .import_remote(head)
            .await
            .map_err(|_| transfer::failed())?;
        let proposal = engine
            .prepare_sync(remote.clone().ok_or_else(unsupported)?)
            .await
            .map_err(|_| transfer::failed())?;
        saved.proposal = Some(proposal);
        saved.save(selection)?;
        observation::project(control, saved, state)?;
        if saved
            .proposal
            .as_ref()
            .is_some_and(|p| !p.conflicts.is_empty())
        {
            return Ok(());
        }
        recovery::apply(engine, selection, saved, control).await?;
    } else {
        engine.capture().await.map_err(|_| transfer::failed())?;
        saved.proposal = None;
        saved.save(selection)?;
    }
    let reader = HistoryReader::open(binding.clone())
        .await
        .map_err(|_| transfer::failed())?;
    let id = uuid::Uuid::new_v4().to_string();
    let intent = reader
        .prepare_push(&transfer, &id, remote)
        .await
        .map_err(|_| transfer::failed())?;
    let counts = reader
        .push_divergence(&intent)
        .await
        .map_err(|_| transfer::failed())?;
    publication.review = Some(backup_journal::Review {
        native: intent.clone(),
        counts,
        started: true,
    });
    publication.save(selection)?;
    control.update(|s| s.phase = RepositorySyncPhase::Publishing)?;
    let outcome = async {
        transfer.reserve(&intent).map_err(|_| transfer::failed())?;
        transfer
            .publish(&intent)
            .await
            .map_err(|_| transfer::failed())
    }
    .await;
    if outcome.is_err() && operations::pending(&binding)?.is_none() {
        publication.review = None;
        publication.save(selection)?;
    }
    match outcome? {
        PushOutcome::Complete => {
            publication.review = None;
            publication.observation = None;
            publication.save(selection)?;
            saved.last_success = Some(chrono::Utc::now().timestamp_millis());
            saved.proposal = None;
            saved.save(selection)?;
            Ok(())
        }
        PushOutcome::Stale => {
            publication.review = None;
            publication.save(selection)?;
            Err(Error::new(
                ErrorCode::Conflict,
                "Remote backup advanced; synchronize again",
            ))
        }
        _ => Err(transfer::failed()),
    }
}
