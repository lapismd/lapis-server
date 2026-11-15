use super::*;
use crate::services::backup::dto::checkpoint;
pub(super) fn proposal(value: &jj_vault::SyncProposal) -> RepositorySyncProposal {
    RepositorySyncProposal {
        local: checkpoint(value.local.clone()),
        incoming: checkpoint(value.incoming.clone()),
        candidate: value.candidate.clone().map(checkpoint),
        conflicts: value.conflicts.clone(),
    }
}
pub(super) fn project(
    control: &Control,
    saved: &journal::Journal,
    state: &SelectionState,
) -> Result<()> {
    control.update(|s| populate(s, saved, state))
}
pub(super) fn populate(
    s: &mut RepositorySyncSnapshot,
    saved: &journal::Journal,
    state: &SelectionState,
) {
    s.revision = state.revision;
    s.enabled = state.automatic_sync;
    s.proposal = saved.proposal.as_ref().map(proposal);
    s.recovery = saved.pending.is_some();
    s.pending_apply = saved.pending.as_ref().map(|r| r.request_id.clone());
    s.last_success = saved.last_success;
    s.phase = if s.recovery {
        RepositorySyncPhase::Attention
    } else if saved
        .proposal
        .as_ref()
        .is_some_and(|p| !p.conflicts.is_empty())
    {
        RepositorySyncPhase::Conflicts
    } else if s.enabled {
        RepositorySyncPhase::Ready
    } else {
        RepositorySyncPhase::Paused
    };
}
