use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[ts(tag = "kind", rename_all = "snake_case")]
pub enum TerminalCommand {
    Create {
        cwd: Option<String>,
        cols: u16,
        rows: u16,
    },
    Write {
        session_id: Uuid,
        data: String,
    },
    Resize {
        session_id: Uuid,
        cols: u16,
        rows: u16,
    },
    Stop {
        session_id: Uuid,
    },
    CloseAll {},
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[ts(tag = "kind", rename_all = "snake_case")]
pub enum TerminalValue {
    Created { session_id: Uuid },
    Done {},
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct TerminalOutput {
    #[ts(type = "number")]
    pub cursor: u64,
    pub bytes: Vec<u8>,
    pub truncated: bool,
    pub running: bool,
    pub exit_code: Option<i32>,
}
