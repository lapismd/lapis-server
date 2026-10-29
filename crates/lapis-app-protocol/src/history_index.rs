//! Host-neutral immutable History index controls. No renderer batch scheduler.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
#[ts(tag = "type", rename_all = "snake_case")]
pub enum HistoryExpression {
    Term {
        field: Option<String>,
        value: String,
    },
    And {
        clauses: Vec<HistoryExpression>,
    },
    Or {
        clauses: Vec<HistoryExpression>,
    },
    Not {
        operand: Box<HistoryExpression>,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct HistoryIndexQuery {
    pub expression: HistoryExpression,
    pub case_sensitive: bool,
    pub cursor: Option<String>,
    pub limit: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[ts(tag = "kind", rename_all = "snake_case")]
pub enum HistoryIndexCommand {
    Start {},
    Status {},
    Query {
        search: HistoryIndexQuery,
    },
    Context {
        commit: String,
        path: String,
        start: usize,
        end: usize,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct HistoryIndexProgress {
    #[ts(type = "number")]
    pub generation: u64,
    #[ts(type = "number")]
    pub indexed: u64,
    #[ts(type = "number")]
    pub omitted: u64,
    pub complete: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct HistoryCheckpoint {
    pub operation: String,
    pub commit: String,
    pub change_id: String,
    pub description: String,
    #[ts(type = "number")]
    pub timestamp_millis: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct HistorySavedRevision {
    pub checkpoint: HistoryCheckpoint,
    pub author: String,
    pub email: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct HistoryIndexRecord {
    pub revision: HistorySavedRevision,
    pub parents: Vec<String>,
    pub path: Option<String>,
    pub content_id: Option<String>,
    pub deleted: bool,
    pub previous: Option<String>,
    pub unavailable: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct HistoryIndexHit {
    pub record: HistoryIndexRecord,
    pub excerpt: String,
    pub line: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct HistoryIndexPage {
    pub status: HistoryIndexProgress,
    pub hits: Vec<HistoryIndexHit>,
    pub next: Option<String>,
    pub reset: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct HistoryIndexContext {
    pub text: String,
    pub start: usize,
    pub end: usize,
    pub before: bool,
    pub after: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(untagged)]
#[ts(untagged)]
pub enum HistoryIndexValue {
    Status(HistoryIndexProgress),
    Page(HistoryIndexPage),
    Context(HistoryIndexContext),
}
