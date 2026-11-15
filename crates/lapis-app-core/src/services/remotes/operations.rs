//! Original transfers keep working files and history-reader pins untouched.
use super::{
    journal::{self, Journal, Review},
    *,
};
use crate::services::history::source_access::SourceAccess;
use jj_store_jj::original_remotes::OriginalRepository;
use jj_vault::{Binding, PushOutcome};
use transfer::{failed, original_error};

pub(super) fn run(
    vault: &Arc<VaultService>,
    storage: &RemoteStorage,
    selection: &Selection,
    command: RepositoryRemotesCommand,
    runtime: &tokio::runtime::Runtime,
) -> Result<()> {
    use RepositoryRemotesCommand as Command;
    let (expected, target) = match &command {
        Command::TestConnection {
            expected_revision,
            target,
        }
        | Command::Fetch {
            expected_revision,
            target,
        }
        | Command::ReviewPush {
            expected_revision,
            target,
        }
        | Command::Push {
            expected_revision,
            target,
            ..
        }
        | Command::ObservePush {
            expected_revision,
            target,
            ..
        }
        | Command::StopPush {
            expected_revision,
            target,
            ..
        } => (*expected_revision, *target),
        _ => return Err(unsupported()),
    };
    let (_guard, state) = selection.guard(expected)?;
    let connection = state.connection(target).ok_or_else(|| {
        Error::new(
            ErrorCode::InvalidRequest,
            "Save a connection for this repository first",
        )
    })?;
    if target != RemoteRepositoryTarget::Original {
        return runtime.block_on(backup_operations::run(
            vault, storage, selection, &state, command,
        ));
    }
    runtime.block_on(async {
        let access = SourceAccess::read(vault, target).map_err(|_| unsupported())?;
        let source = access.fingerprint();
        let root = access.root().to_path_buf();
        if state.original_binding.as_ref() != Some(&source) {
            return Err(Error::new(
                ErrorCode::Conflict,
                "Original repository changed; save its connection again",
            ));
        }
        let mut repository = OriginalRepository::open(Arc::new(original::Writer(access)), root)
            .await
            .map_err(original_error)?;
        let discovery = repository.snapshot().await.map_err(original_error)?;
        if !state.original_observed.as_ref().is_some_and(|observed| {
            observed.source == source && observed.configuration == discovery.remote_configuration
        }) {
            return Err(Error::new(
                ErrorCode::Conflict,
                "Original remote configuration changed; refresh before continuing",
            ));
        }
        let binding = repository
            .transport_binding(selection.private_path())
            .map_err(original_error)?;
        let pending = pending(&binding)?;
        // Stopping only retires local tracking, even if credentials are locked.
        if let Command::StopPush { request_id, .. } = &command {
            require_pending(pending.as_ref(), request_id)?;
            binding.stop_publication(request_id).map_err(|_| failed())?;
            return Ok(());
        }
        if pending.is_some() && !matches!(command, Command::ObservePush { .. }) {
            return Err(Error::new(
                ErrorCode::Busy,
                "Observe or stop the earlier publication before continuing",
            ));
        }
        let mut journal =
            Journal::open(selection, &state)?.map_or_else(|| Journal::new(&state), Ok)?;
        match &command {
            Command::ReviewPush { .. } => require_push(connection)?,
            Command::Push { review_id, .. } => {
                require_push(connection)?;
                if journal
                    .review
                    .as_ref()
                    .is_none_or(|r| r.id != *review_id || r.started)
                {
                    return Err(Error::new(
                        ErrorCode::Conflict,
                        "Review this publication again before pushing",
                    ));
                }
            }
            Command::ObservePush { request_id, .. } => {
                require_pending(pending.as_ref(), request_id)?
            }
            _ => {}
        }
        let transfer = transfer::connect(storage, binding, connection).await?;
        match command {
            Command::TestConnection { .. } => {
                // Reachability/authentication only: no fetch and no publication.
                transfer.observe().await.map_err(|_| failed())?;
            }
            Command::Fetch { .. } => {
                repository
                    .fetch(&connection.name, &transfer)
                    .await
                    .map_err(original_error)?;
                let counts = repository
                    .divergence(&connection.name, &connection.branch, &connection.bookmark)
                    .await
                    .map_err(original_error)?;
                journal.observation = Some(RemoteObservation {
                    incoming: journal::count(counts.incoming)?,
                    outgoing: journal::count(counts.outgoing)?,
                    fetched_at: chrono::Utc::now().to_rfc3339(),
                });
                journal.review = None;
                journal.save(selection)?;
            }
            Command::ReviewPush { .. } => {
                require_push(connection)?;
                let id = uuid::Uuid::new_v4().to_string();
                let native = repository
                    .review_push(&connection.name, &connection.bookmark, &id, &transfer)
                    .await
                    .map_err(original_error)?;
                journal.review = Some(Review {
                    id,
                    native,
                    started: false,
                });
                journal.save(selection)?;
            }
            Command::Push { review_id, .. } => {
                require_push(connection)?;
                let review = journal
                    .review
                    .as_mut()
                    .filter(|r| r.id == review_id && !r.started)
                    .ok_or_else(|| {
                        Error::new(
                            ErrorCode::Conflict,
                            "Review this publication again before pushing",
                        )
                    })?;
                let native = review.native.clone();
                review.started = true;
                journal.save(selection)?;
                // Native rechecks operation, bookmark, endpoint and ancestry,
                // then durably reserves the request before starting any push.
                match repository
                    .push(&native, &transfer)
                    .await
                    .map_err(original_error)?
                {
                    PushOutcome::Complete => {
                        journal.observation = None;
                        journal.save(selection)?;
                    }
                    PushOutcome::Stale => {
                        return Err(Error::new(
                            ErrorCode::Conflict,
                            "Remote rejected this update; fetch and review again",
                        ));
                    }
                    _ => {}
                }
            }
            Command::ObservePush { request_id, .. } => {
                require_pending(pending.as_ref(), &request_id)?;
                transfer
                    .reconcile(&request_id)
                    .await
                    .map_err(|_| failed())?;
            }
            _ => return Err(unsupported()),
        }
        Ok(())
    })
}
pub(super) fn pending(binding: &Binding) -> Result<Option<jj_vault::PendingPublication>> {
    // Native lookup admits a genuinely absent receipt, but refuses an orphaned
    // active receipt even when its binding record is missing or damaged.
    binding.pending_publication().map_err(|_| failed())
}
pub(super) fn require_pending(
    pending: Option<&jj_vault::PendingPublication>,
    id: &str,
) -> Result<()> {
    if pending.is_none_or(|p| p.request_id != id) {
        return Err(Error::new(
            ErrorCode::Conflict,
            "Pending publication changed; refresh before continuing",
        ));
    }
    Ok(())
}
pub(super) fn require_push(connection: &RemoteConnection) -> Result<()> {
    if connection.authentication == RemoteAuthentication::Anonymous {
        return Err(Error::new(
            ErrorCode::PermissionDenied,
            "Choose authentication before publishing",
        ));
    }
    Ok(())
}
