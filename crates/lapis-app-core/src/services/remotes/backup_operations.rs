//! One reviewed saved checkpoint; transport and capture retain separate ownership.
use super::{
    backup_journal::{Journal, Review},
    *,
};
use jj_store_jj::vault_workspace::{CommitDivergence, HistoryReader};
use jj_vault::{PushOutcome, RemoteTransfer};
use transfer::failed;

pub(super) async fn run(
    vault: &Arc<VaultService>,
    storage: &RemoteStorage,
    selection: &Selection,
    state: &SelectionState,
    command: RepositoryRemotesCommand,
) -> Result<()> {
    use RepositoryRemotesCommand as Command;
    let connection = state.backup.as_ref().ok_or_else(unsupported)?;
    let binding = backup::binding(vault)?;
    let pending = operations::pending(&binding)?;
    if let Command::StopPush { request_id, .. } = &command {
        operations::require_pending(pending.as_ref(), request_id)?;
        binding.stop_publication(request_id).map_err(|_| failed())?;
        let mut journal = Journal::open(selection, state, &backup::source(vault)?)?;
        journal.review = None;
        journal.save(selection)?;
        return Ok(());
    }
    if pending.is_some() && !matches!(command, Command::ObservePush { .. }) {
        return Err(Error::new(
            ErrorCode::Busy,
            "Observe or stop the earlier backup publication before continuing",
        ));
    }
    let mut journal = Journal::open(selection, state, &backup::source(vault)?)?;
    match &command {
        Command::ReviewPush { .. } => operations::require_push(connection)?,
        Command::Push { review_id, .. } => {
            operations::require_push(connection)?;
            if journal
                .review
                .as_ref()
                .is_none_or(|r| r.native.request_id() != review_id || r.started)
            {
                return Err(Error::new(
                    ErrorCode::Conflict,
                    "Review this backup publication again before pushing",
                ));
            }
        }
        Command::ObservePush { request_id, .. } => {
            operations::require_pending(pending.as_ref(), request_id)?
        }
        _ => {}
    }
    let transfer = transfer::connect(storage, binding.clone(), connection).await?;
    match command {
        Command::TestConnection { .. } => {
            transfer.observe().await.map_err(|_| failed())?;
        }
        Command::Fetch { .. } => {
            let remote = fetched(vault, &transfer).await?;
            let reader = HistoryReader::open(binding).await.map_err(|_| failed())?;
            journal.observation = reader
                .checkpoint_divergence(remote.as_deref())
                .await
                .map_err(|_| failed())?
                .map(|counts| observation(&counts))
                .transpose()?;
            journal.review = None;
            journal.save(selection)?;
        }
        Command::ReviewPush { .. } => {
            let remote = fetched(vault, &transfer).await?;
            let reader = HistoryReader::open(binding).await.map_err(|_| failed())?;
            let id = uuid::Uuid::new_v4().to_string();
            let native = reader.prepare_push(&transfer, &id, remote).await.map_err(|_| Error::new(
                ErrorCode::Conflict, "Backup history is unavailable or diverged; fetch and reconcile before publishing",
            ))?;
            let counts = reader
                .push_divergence(&native)
                .await
                .map_err(|_| failed())?;
            journal.observation = Some(observation(&counts)?);
            journal.review = Some(Review {
                native,
                counts,
                started: false,
            });
            journal.save(selection)?;
        }
        Command::Push { .. } => {
            let review = journal.review.as_mut().ok_or_else(failed)?;
            let native = review.native.clone();
            review.started = true;
            journal.save(selection)?;
            let outcome = async {
                transfer.reserve(&native).map_err(|_| failed())?;
                transfer.publish(&native).await.map_err(|_| failed())
            }
            .await;
            if outcome.is_err() && operations::pending(&binding)?.is_none() {
                journal.review = None;
                journal.save(selection)?;
            }
            match outcome? {
                PushOutcome::Complete => {
                    journal.observation = None;
                    journal.review = None;
                    journal.save(selection)?;
                }
                PushOutcome::Stale => {
                    journal.review = None;
                    journal.save(selection)?;
                    return Err(Error::new(
                        ErrorCode::Conflict,
                        "Remote backup changed; fetch and review again",
                    ));
                }
                _ => {}
            }
        }
        Command::ObservePush { request_id, .. } => {
            let outcome = transfer
                .reconcile(&request_id)
                .await
                .map_err(|_| failed())?;
            if matches!(
                outcome,
                PushOutcome::Complete | PushOutcome::Stale | PushOutcome::Stopped
            ) {
                journal.review = None;
                journal.save(selection)?;
            }
        }
        _ => return Err(unsupported()),
    }
    Ok(())
}
async fn fetched(vault: &Arc<VaultService>, transfer: &RemoteTransfer) -> Result<Option<String>> {
    let fetched = transfer.fetch().await.map_err(|_| failed())?;
    let remote = fetched.as_ref().map(|head| head.commit().to_owned());
    if let Some(head) = fetched {
        crate::services::backup::remote::import(vault, head).await?;
    }
    Ok(remote)
}
fn observation(counts: &CommitDivergence) -> Result<RemoteObservation> {
    Ok(RemoteObservation {
        incoming: journal::count(counts.incoming)?,
        outgoing: journal::count(counts.outgoing)?,
        fetched_at: chrono::Utc::now().to_rfc3339(),
    })
}
