use lapis_app_protocol::{repository_backup::*, repository_restore::*};

pub(super) fn checkpoint(value: jj_vault::Checkpoint) -> RepositoryCheckpoint {
    RepositoryCheckpoint {
        operation: value.operation,
        commit: value.commit,
        change_id: value.change_id,
        description: value.description,
        timestamp_millis: value.timestamp_millis,
    }
}
pub(super) fn capture(value: jj_vault::Capture) -> RepositoryRestoreValue {
    RepositoryRestoreValue::Captured(RepositoryCapture {
        changed: value.changed,
        checkpoint: value.checkpoint.map(checkpoint),
        operation: value.operation,
        working_copy: value.working_copy,
    })
}
pub(super) fn status(value: jj_vault::Status) -> RepositoryBackupStatus {
    RepositoryBackupStatus {
        phase: match value.phase {
            jj_vault::Phase::Saving => RepositoryBackupPhase::Saving,
            jj_vault::Phase::Ready => RepositoryBackupPhase::Ready,
            jj_vault::Phase::Failed => RepositoryBackupPhase::Failed,
            jj_vault::Phase::Closed => RepositoryBackupPhase::Closed,
        },
        checkpoint: value.checkpoint.map(checkpoint),
        error: value.error.map(|error| error.chars().take(2048).collect()),
    }
}
