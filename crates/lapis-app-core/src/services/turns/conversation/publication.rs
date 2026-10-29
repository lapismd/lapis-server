use super::super::{Error, Result, Work};
use super::line;
use crate::turns::{TurnOutcome, participant_capture};
use lapis_app_protocol::{ErrorCode, ExpectedVersion};
use serde_json::{Value, json};
use uuid::Uuid;

pub(super) struct Slot {
    pub assistant: Value,
    pub delivery: Option<Value>,
    pub begin: Vec<Value>,
    pub terminal: Option<Value>,
    pub outcome: Option<Value>,
    pub receipt: Option<Value>,
    pub finished: bool,
}
pub(super) struct Publication {
    pub directory: String,
    pub metadata: Value,
    pub metadata_version: String,
    pub agents_version: String,
    pub transcript_version: String,
    pub prior: String,
    pub user: Value,
    pub slots: Vec<Slot>,
    pub dispatch: Option<Value>,
    pub blocked: bool,
}
fn conflict() -> Error {
    Error::new(
        ErrorCode::Conflict,
        "Native conversation publication is fenced",
    )
}
fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
impl Publication {
    pub fn verify(&self, work: &Work) -> Result<()> {
        if self.blocked {
            return Err(conflict());
        }
        if self.dispatch.is_some() {
            for (name, version) in [
                ("metadata.yaml", &self.metadata_version),
                ("agents.jsonl", &self.agents_version),
            ] {
                if work
                    .read_text(&format!("{}/{name}", self.directory))?
                    .version
                    != *version
                {
                    return Err(conflict());
                }
            }
        }
        Ok(())
    }
    pub fn text(&self) -> String {
        let mut text = format!("{}{}", self.prior, line(&self.user));
        for slot in &self.slots {
            for value in &slot.begin {
                text.push_str(&line(value));
            }
            if slot.assistant["text"]
                .as_str()
                .is_some_and(|s| !s.is_empty())
            {
                text.push_str(&line(&slot.assistant));
            }
            for value in [&slot.terminal, &slot.outcome, &slot.receipt]
                .into_iter()
                .flatten()
            {
                text.push_str(&line(value));
            }
        }
        text
    }
    fn write_transcript(&mut self, work: &Work) -> Result<()> {
        self.transcript_version = work.write_text(
            &format!("{}/transcript.jsonl", self.directory),
            &self.text(),
            &ExpectedVersion::Version {
                version: self.transcript_version.clone(),
            },
        )?;
        Ok(())
    }
    fn write_metadata(&mut self, work: &Work) -> Result<()> {
        self.metadata_version = work.write_text(
            &format!("{}/metadata.yaml", self.directory),
            &line(&self.metadata),
            &ExpectedVersion::Version {
                version: self.metadata_version.clone(),
            },
        )?;
        Ok(())
    }
    pub fn started(&mut self, index: usize, work: &Work) -> Result<()> {
        let Some(dispatch) = self.dispatch.clone() else {
            return Ok(());
        };
        let slot = &mut self.slots[index];
        let delivery = slot.delivery.as_mut().expect("participant");
        if delivery["state"] != "pending" {
            return Err(conflict());
        }
        delivery["state"] = "started".into();
        delivery["agentBindingId"] = slot.assistant["agentBindingId"].clone();
        slot.begin.push(
            participant_capture::delivery_entry(&dispatch, delivery, &now())
                .map_err(|e| super::super::participants::invalid(&e.message))?,
        );
        self.metadata["pendingDispatch"]["deliveries"][index] = delivery.clone();
        self.write_metadata(work)?;
        self.write_transcript(work)
    }
    pub fn progress(&mut self, index: usize, work: &Work, output: &str) -> Result<()> {
        if self.slots[index].finished {
            return Err(conflict());
        }
        self.slots[index].assistant["text"] = output.into();
        self.write_transcript(work)
    }
    pub fn undispatched(&mut self, index: usize, work: &Work, cancelled: bool) -> Result<()> {
        let dispatch = self.dispatch.clone().ok_or_else(conflict)?;
        let time = now();
        self.metadata["updatedAt"] = time.clone().into();
        self.write_metadata(work)?;
        let slot = &mut self.slots[index];
        let delivery = slot.delivery.as_mut().ok_or_else(conflict)?;
        if slot.finished || delivery["state"] != "pending" {
            return Err(conflict());
        }
        delivery["state"] = if cancelled { "cancelled" } else { "not_sent" }.into();
        slot.outcome = Some(
            participant_capture::delivery_entry(&dispatch, delivery, &time)
                .map_err(|e| super::super::participants::invalid(&e.message))?,
        );
        slot.finished = true;
        self.write_transcript(work)
    }
    pub fn finish(
        &mut self,
        index: usize,
        work: &Work,
        outcome: &TurnOutcome,
        receipt: Option<Value>,
    ) -> Result<()> {
        if self.slots[index].finished {
            return Err(conflict());
        }
        let time = now();
        self.metadata["updatedAt"] = time.clone().into();
        // Keep started metadata as cold evidence until immutable outcomes exist.
        // No metadata write can fail after a successful participant outcome.
        if self.dispatch.is_some() {
            self.write_metadata(work)?;
        }
        let slot = &mut self.slots[index];
        slot.terminal = match outcome {
            TurnOutcome::Completed(_) => None,
            TurnOutcome::Cancelled => Some(
                json!({"schemaVersion":3,"id":Uuid::new_v4(),"type":"cancelled","createdAt":time,"text":"Native turn cancelled"}),
            ),
            TurnOutcome::Failed(code) | TurnOutcome::Unknown(code) => Some(
                json!({"schemaVersion":3,"id":Uuid::new_v4(),"type":"error","createdAt":time,"message":code,"retryable":false}),
            ),
        };
        if let Some(dispatch) = &self.dispatch {
            if let Some(terminal) = &mut slot.terminal {
                terminal["parentId"] = self.user["id"].clone();
                terminal["agentBindingId"] = slot.assistant["agentBindingId"].clone();
            }
            let delivery = slot.delivery.as_mut().expect("participant");
            delivery["state"] = match outcome {
                TurnOutcome::Completed(_) => "completed",
                TurnOutcome::Failed(_) => "rejected",
                _ => "unknown",
            }
            .into();
            slot.outcome = Some(
                participant_capture::delivery_entry(dispatch, delivery, &time)
                    .map_err(|e| super::super::participants::invalid(&e.message))?,
            );
        }
        slot.receipt = receipt;
        slot.finished = true;
        self.write_transcript(work)?;
        if self.dispatch.is_none() {
            self.write_metadata(work)?;
        }
        Ok(())
    }
}
