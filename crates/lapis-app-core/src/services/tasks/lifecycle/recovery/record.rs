use super::*;
use crate::record_store::StoredRecord;
use lapis_task_services::{
    TaskLifecyclePlan, TaskOccurrenceCompletion, TaskOccurrenceMaterialization, plan_task_lifecycle,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskLifecycleRequest {
    pub request_id: Uuid,
    pub path: String,
    pub expected_version: String,
    #[serde(deserialize_with = "strict")]
    pub action: TaskLifecycleAction,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskLifecyclePolicy {
    #[serde(deserialize_with = "strict")]
    pub day_parts: lapis_task_services::TaskDayPartSettings,
    pub daily: DailyDocumentSettings,
    pub format: DailyFormatContext,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Observation {
    #[serde(deserialize_with = "strict")]
    pub input: TaskOccurrenceMaterialization,
    #[serde(deserialize_with = "strict")]
    pub completion: Option<TaskOccurrenceCompletion>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Plan {
    pub task_raw: String,
    pub observation: Option<Observation>,
}
impl From<TaskLifecyclePlan> for Plan {
    fn from(value: TaskLifecyclePlan) -> Self {
        Self {
            task_raw: value.task_raw,
            observation: value.observation.map(|value| Observation {
                input: value.input,
                completion: value.completion,
            }),
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Intent {
    pub path: String,
    pub before: Option<TextSnapshot>,
    pub after: String,
}
impl Intent {
    pub fn desired(&self) -> TextSnapshot {
        snapshot(&self.path, &self.after)
    }
    fn validate(&self) -> io::Result<()> {
        if self.path.is_empty()
            || self
                .path
                .split('/')
                .any(|part| part.is_empty() || part.starts_with('.'))
            || !crate::services::indexing::records::markdown(&self.path)
            || self.path.contains(['\\', ':', '\0'])
            || self.before.as_ref().is_some_and(|before| {
                before.path != self.path || before.version != hash(&before.text)
            })
        {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Daily {
    pub ensure: Intent,
    pub observation: Option<Intent>,
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Stage {
    Accepted,
    Ensuring,
    Observing,
    Advancing,
    Completed,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TaskLifecycleFailure {
    pub code: String,
    pub message: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    pub schema_version: u32,
    pub vault_id: Uuid,
    pub request: TaskLifecycleRequest,
    pub policy: TaskLifecyclePolicy,
    pub original: TextSnapshot,
    pub plan: Plan,
    pub daily: Option<Daily>,
    pub stage: Stage,
    pub last_failure: Option<TaskLifecycleFailure>,
}
pub(super) fn hash(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}
pub(super) fn snapshot(path: &str, text: &str) -> TextSnapshot {
    TextSnapshot {
        path: path.into(),
        text: text.into(),
        version: hash(text),
    }
}
pub(super) fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "Invalid native Task lifecycle record",
    )
}
pub(super) fn equal<T: Serialize>(a: &T, b: &T) -> bool {
    serde_json::to_vec(a)
        .ok()
        .zip(serde_json::to_vec(b).ok())
        .is_some_and(|(a, b)| a == b)
}
impl StoredRecord for Record {
    const KIND: &'static str = "Task lifecycle";
    const MAX_RECORD_BYTES: usize = 16 * 1024 * 1024;
    const MAX_RECORDS: usize = 4096;
    fn id(&self) -> Uuid {
        self.request.request_id
    }
    fn validate(&self, vault: Uuid, id: Uuid) -> io::Result<()> {
        if self.schema_version != 1
            || self.vault_id != vault
            || vault.is_nil()
            || id.get_version_num() != 4
            || self.id() != id
            || self.original.path != self.request.path
            || self.original.version != self.request.expected_version
            || self.original.version != hash(&self.original.text)
            || self.last_failure.as_ref().is_some_and(|failure| {
                failure.code.is_empty()
                    || failure.code.len() > 128
                    || failure.code.chars().any(char::is_control)
                    || failure.message.trim().is_empty()
                    || failure.message.len() > 2048
            })
            || matches!(&self.request.action, TaskLifecycleAction::Complete { tracked_value: Some(value), .. } if !value.is_finite())
        {
            return Err(invalid());
        }
        let plan: Plan = plan_task_lifecycle(
            &self.original.text,
            &self.request.path,
            &self.request.action,
            &self.policy.day_parts,
        )
        .map_err(|_| invalid())?
        .into();
        if !equal(&self.plan, &plan) {
            return Err(invalid());
        }
        match (&self.daily, &self.plan.observation, self.stage) {
            (None, _, Stage::Accepted) | (None, None, Stage::Advancing | Stage::Completed) => {}
            (Some(daily), Some(observation), stage) if stage != Stage::Accepted => {
                daily.ensure.validate()?;
                if daily.ensure.path == self.original.path
                    || daily
                        .ensure
                        .before
                        .as_ref()
                        .is_some_and(|before| before.text != daily.ensure.after)
                {
                    return Err(invalid());
                }
                let metadata =
                    lapis_plugin_services::markdown::extract_metadata(&daily.ensure.after);
                if metadata["frontmatter"]["type"] != "daily-note"
                    || metadata["frontmatter"]["date"] != observation.input.occurrence_date
                {
                    return Err(invalid());
                }
                if let Some(intent) = &daily.observation {
                    intent.validate()?;
                    let desired = super::super::super::observations::process(
                        &daily.ensure.after,
                        &daily.ensure.path,
                        &observation.input,
                        observation.completion.as_ref(),
                    )
                    .map_err(|_| invalid())?
                    .0;
                    if stage == Stage::Ensuring
                        || intent.path != daily.ensure.path
                        || !intent
                            .before
                            .as_ref()
                            .is_some_and(|before| before.text == daily.ensure.after)
                        || intent.after != desired
                    {
                        return Err(invalid());
                    }
                } else if stage != Stage::Ensuring {
                    return Err(invalid());
                }
            }
            _ => return Err(invalid()),
        }
        Ok(())
    }
}

fn strict<'de, D: serde::Deserializer<'de>, T: serde::de::DeserializeOwned + Serialize>(
    deserializer: D,
) -> std::result::Result<T, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    let decoded: T = serde_json::from_value(value.clone()).map_err(serde::de::Error::custom)?;
    if serde_json::to_value(&decoded).map_err(serde::de::Error::custom)? != value {
        return Err(serde::de::Error::custom(
            "Non-canonical Task lifecycle input",
        ));
    }
    Ok(decoded)
}
