use crate::ExpectedVersion;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct EditingIntentState {
    pub editor_id: Uuid,
    pub path: String,
    pub generation: u32,
    pub revision: u32,
    pub dirty: bool,
    pub active: bool,
    pub detached: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[ts(tag = "kind", rename_all = "snake_case")]
pub enum EditingIntentCommand {
    Acquire {
        editor_id: Uuid,
        path: String,
    },
    Changed {
        editor_id: Uuid,
        generation: u32,
        revision: u32,
    },
    Yield {
        editor_id: Uuid,
        generation: u32,
        revision: u32,
        expected: ExpectedVersion,
    },
    Resume {
        editor_id: Uuid,
        generation: u32,
    },
    Retire {
        editor_id: Uuid,
        generation: u32,
    },
    Recover {
        editor_id: Uuid,
        generation: u32,
        revision: u32,
    },
    Discard {
        editor_id: Uuid,
        generation: u32,
        revision: u32,
    },
    Inspect {
        editor_id: Uuid,
    },
    List {},
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[ts(tag = "kind", rename_all = "snake_case")]
pub enum EditingIntentValue {
    State { state: EditingIntentState },
    States { states: Vec<EditingIntentState> },
    Retired {},
    Missing {},
}
