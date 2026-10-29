use super::*;
use crate::services::{
    daily::{self, DailyDocumentPlan},
    operations,
};
use inspect::Content;
use record::{Daily, Intent};

impl TaskLifecycleJournal {
    fn prepare_daily(work: &Work, record: &Record) -> Result<(daily::io::Prepared, Intent)> {
        let observation = record
            .plan
            .observation
            .as_ref()
            .ok_or_else(record::invalid)?;
        let prepared = daily::io::prepare(
            work,
            &observation.input.occurrence_date,
            Some((&record.policy.daily, &record.policy.format)),
        )?;
        let intent = match prepared.plan() {
            Some(DailyDocumentPlan::Existing { path }) => {
                let before = prepared.existing(path)?;
                Intent {
                    path: path.clone(),
                    after: before.text.clone(),
                    before: Some(before),
                }
            }
            Some(DailyDocumentPlan::Create { path, text }) => Intent {
                path: path.clone(),
                before: None,
                after: text.clone(),
            },
            None => return Err(record::invalid().into()),
        };
        Ok((prepared, intent))
    }
    pub(super) fn drive(
        &self,
        work: &Work,
        record: &mut Record,
        expected: &mut Vec<u8>,
        checkpoint: &mut impl FnMut(Checkpoint) -> io::Result<()>,
    ) -> Result<()> {
        loop {
            self.verify(work)?;
            if self.status(work, record)?.disposition == TaskLifecycleDisposition::Divergent {
                return Err(conflict().into());
            }
            match record.stage {
                Stage::Accepted if record.plan.observation.is_some() => {
                    // Recheck the original before even resolving a fresh daily target.
                    super::super::original(
                        work,
                        &record.original.path,
                        &ExpectedVersion::Version {
                            version: record.original.version.clone(),
                        },
                    )?;
                    let (_, ensure) = Self::prepare_daily(work, record)?;
                    record.daily = Some(Daily {
                        ensure,
                        observation: None,
                    });
                    record.stage = Stage::Ensuring;
                    self.publish(record, expected)?;
                    checkpoint(Checkpoint::Ensuring)?;
                }
                Stage::Accepted => {
                    record.stage = Stage::Advancing;
                    self.publish(record, expected)?;
                    checkpoint(Checkpoint::Advancing)?;
                }
                Stage::Ensuring => {
                    let ensure = &record.daily.as_ref().ok_or_else(record::invalid)?.ensure;
                    match Self::content(work, ensure)? {
                        Content::Before => {
                            let (prepared, current) = Self::prepare_daily(work, record)?;
                            if !record::equal(&current, ensure) {
                                return Err(conflict().into());
                            }
                            daily::io::finish(work, prepared)?.ok_or_else(record::invalid)?;
                        }
                        Content::Desired => {}
                        Content::Divergent => return Err(conflict().into()),
                    }
                    checkpoint(Checkpoint::Ensured)?;
                    let daily = record.daily.as_mut().ok_or_else(record::invalid)?;
                    let observation = record
                        .plan
                        .observation
                        .as_ref()
                        .ok_or_else(record::invalid)?;
                    // Preserve writer-admission precedence over source transform validation.
                    {
                        let state = work.vault.lock()?;
                        operations::guard_existing_write(
                            &state,
                            &daily.ensure.path,
                            &ExpectedVersion::Version {
                                version: record::hash(&daily.ensure.after),
                            },
                            None,
                        )?;
                    }
                    // Quantitative validation intentionally runs after ensure, as in the source.
                    let (after, _) = super::super::super::observations::process(
                        &daily.ensure.after,
                        &daily.ensure.path,
                        &observation.input,
                        observation.completion.as_ref(),
                    )
                    .map_err(super::super::super::TaskWriteError::from)?;
                    daily.observation = Some(Intent {
                        path: daily.ensure.path.clone(),
                        before: Some(daily.ensure.desired()),
                        after,
                    });
                    record.stage = Stage::Observing;
                    self.publish(record, expected)?;
                    checkpoint(Checkpoint::Observing)?;
                }
                Stage::Observing => {
                    let observation = record
                        .daily
                        .as_ref()
                        .and_then(|daily| daily.observation.as_ref())
                        .ok_or_else(record::invalid)?;
                    Self::apply(work, observation)?;
                    checkpoint(Checkpoint::Observed)?;
                    record.stage = Stage::Advancing;
                    self.publish(record, expected)?;
                    checkpoint(Checkpoint::Advancing)?;
                }
                Stage::Advancing => {
                    Self::apply(work, &Self::task_intent(record))?;
                    checkpoint(Checkpoint::Advanced)?;
                    record.stage = Stage::Completed;
                    self.publish(record, expected)?;
                }
                Stage::Completed => return Ok(()),
            }
        }
    }
    fn apply(work: &Work, intent: &Intent) -> Result<()> {
        let before = intent.before.as_ref().ok_or_else(record::invalid)?;
        match Self::content(work, intent)? {
            Content::Desired => Ok(()),
            Content::Divergent => Err(conflict().into()),
            Content::Before => {
                let state = work.vault.lock()?;
                if state.phase == VaultPhase::Closed {
                    return Err(Error::closed().into());
                }
                operations::write_if_changed(
                    &state,
                    &intent.path,
                    &intent.after,
                    &ExpectedVersion::Version {
                        version: before.version.clone(),
                    },
                    None,
                )?;
                Ok(())
            }
        }
    }
}
