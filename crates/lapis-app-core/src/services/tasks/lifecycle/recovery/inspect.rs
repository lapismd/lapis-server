use super::*;
use crate::services::operations;
use record::Intent;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Content {
    Before,
    Desired,
    Divergent,
}
impl TaskLifecycleJournal {
    pub fn inspect(&self, work: &Work, id: Uuid) -> Result<Option<TaskLifecycleStatus>> {
        self.verify(work)?;
        self.storage
            .read(id)?
            .map(|(record, _)| self.status(work, &record))
            .transpose()
    }
    pub(super) fn content(work: &Work, intent: &Intent) -> Result<Content> {
        let state = work.vault.lock()?;
        if state.phase == VaultPhase::Closed {
            return Err(Error::closed().into());
        }
        let current = match operations::read(&state, &intent.path) {
            Ok(value) => Some(value),
            Err(error) if error.code() == ErrorCode::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        // Before wins for a no-op, preserving its writer checks on explicit retry.
        if current.as_ref().map(|value| &value.text)
            == intent.before.as_ref().map(|value| &value.text)
        {
            return Ok(Content::Before);
        }
        if current
            .as_ref()
            .is_some_and(|value| value.text == intent.after)
        {
            return Ok(Content::Desired);
        }
        Ok(Content::Divergent)
    }
    pub(super) fn task_intent(record: &Record) -> Intent {
        Intent {
            path: record.original.path.clone(),
            before: Some(record.original.clone()),
            after: record.plan.task_raw.clone(),
        }
    }
    pub(super) fn status(&self, work: &Work, record: &Record) -> Result<TaskLifecycleStatus> {
        self.verify(work)?;
        let mut disposition = TaskLifecycleDisposition::Retryable;
        if record.stage == Stage::Completed {
            disposition = TaskLifecycleDisposition::Completed;
        } else {
            let original = Self::content(work, &Self::task_intent(record))?;
            if original == Content::Divergent
                || (original == Content::Desired && record.stage != Stage::Advancing)
            {
                disposition = TaskLifecycleDisposition::Divergent;
            }
            if let Some(daily) = &record.daily {
                let intent = daily.observation.as_ref().unwrap_or(&daily.ensure);
                let content = Self::content(work, intent)?;
                let applied = content == Content::Desired
                    || (content == Content::Before
                        && intent
                            .before
                            .as_ref()
                            .is_some_and(|before| before.text == intent.after));
                if content == Content::Divergent || (record.stage == Stage::Advancing && !applied) {
                    disposition = TaskLifecycleDisposition::Divergent;
                }
            }
            if record.stage == Stage::Advancing
                && original == Content::Desired
                && disposition != TaskLifecycleDisposition::Divergent
            {
                disposition = TaskLifecycleDisposition::Completed;
            }
        }
        let daily = match &record.daily {
            None => None,
            Some(daily) if record.stage == Stage::Completed => Some(
                daily
                    .observation
                    .as_ref()
                    .unwrap_or(&daily.ensure)
                    .desired(),
            ),
            Some(daily) => match work.read_text(&daily.ensure.path) {
                Ok(snapshot) => Some(snapshot),
                Err(error) if error.code() == ErrorCode::NotFound => None,
                Err(error) => return Err(error.into()),
            },
        };
        Ok(TaskLifecycleStatus {
            request: record.request.clone(),
            last_failure: record.last_failure.clone(),
            disposition,
            original: record.original.clone(),
            task_replacement: record.plan.task_raw.clone(),
            daily,
            daily_replacement: record.daily.as_ref().map(|daily| {
                daily
                    .observation
                    .as_ref()
                    .unwrap_or(&daily.ensure)
                    .after
                    .clone()
            }),
        })
    }
}
