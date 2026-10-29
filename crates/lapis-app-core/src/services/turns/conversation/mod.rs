mod create;
mod publication;
use super::{Result, Work, history::History, participants::ParticipantMessage};
use crate::turns::TurnOutcome;
use lapis_app_protocol::{ExpectedVersion, turns::TurnRecord};
use publication::Publication;
use serde_json::Value;
use std::sync::{Arc, Mutex};

/// A worker owns one slot; only the shared publication owns file revisions.
#[derive(Clone)]
pub(super) struct Conversation {
    shared: Arc<Mutex<Publication>>,
    index: usize,
}
fn timestamp(ms: u64) -> String {
    chrono::DateTime::from_timestamp_millis(ms as i64)
        .expect("validated native timestamp")
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
fn line(value: &Value) -> String {
    format!("{value}\n")
}
fn expected(version: Option<String>) -> ExpectedVersion {
    version.map_or(ExpectedVersion::Missing {}, |version| {
        ExpectedVersion::Version { version }
    })
}
impl Conversation {
    pub fn create(
        work: &Work,
        record: &TurnRecord,
        history: Option<History>,
        authored: Option<Value>,
    ) -> Result<Self> {
        Ok(create::open(work, record, history, authored, None)?.remove(0))
    }
    pub fn participants(
        work: &Work,
        record: &TurnRecord,
        history: Option<History>,
        message: &ParticipantMessage,
    ) -> Result<Vec<Self>> {
        create::open(
            work,
            record,
            history,
            Some(message.input.user.clone()),
            Some(message),
        )
    }
    fn update(
        &mut self,
        work: &Work,
        action: impl FnOnce(&mut Publication, usize, &Work) -> Result<()>,
    ) -> Result<()> {
        let mut shared = self.shared.lock().map_err(|_| super::failure())?;
        let result = shared
            .verify(work)
            .and_then(|()| action(&mut shared, self.index, work));
        if result.is_err() {
            shared.blocked = true;
        }
        result
    }
    pub fn started(&mut self, work: &Work) -> Result<()> {
        self.update(work, |p, index, work| p.started(index, work))
    }
    pub fn progress(&mut self, work: &Work, output: &str) -> Result<()> {
        self.update(work, |p, index, work| p.progress(index, work, output))
    }
    pub fn undispatched(&mut self, work: &Work, cancelled: bool) -> Result<()> {
        self.update(work, |p, index, work| {
            p.undispatched(index, work, cancelled)
        })
    }
    pub fn finish(
        &mut self,
        work: &Work,
        outcome: &TurnOutcome,
        receipt: Option<Value>,
    ) -> Result<()> {
        self.update(work, |p, index, work| {
            p.finish(index, work, outcome, receipt)
        })
    }
}
