use super::enums::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct MemorySource {
    pub source_key: String,
    pub source_path: String,
    pub source_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub last_entry_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub last_entry_hash: Option<String>,
    pub status: MemorySourceStatus,
    pub indexed_at: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error_code: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct MemoryCandidate {
    pub id: String,
    pub scope_kind: MemoryScope,
    pub scope_path: String,
    pub kind: MemoryKind,
    pub normalized_claim: String,
    pub claim_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub supersession_key: Option<String>,
    pub origin_class: MemoryOrigin,
    #[schemars(range(min = 1, max = 5))]
    #[ts(type = "1 | 2 | 3 | 4 | 5")]
    #[serde(deserialize_with = "importance")]
    pub importance: u8,
    pub triggers: Vec<String>,
    pub state: MemoryCandidateState,
    pub first_seen_at: f64,
    pub last_seen_at: f64,
    pub recurrence_count: f64,
    pub conversation_count: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub promoted_memory_id: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct MemoryCandidateOrigin {
    pub candidate_id: String,
    pub conversation_id: String,
    pub entry_id: String,
    pub entry_hash: String,
    pub observed_at: f64,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct MemoryCandidateInput {
    pub candidate: MemoryCandidate,
    pub origins: Vec<MemoryCandidateOrigin>,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct MemoryCandidateQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub scope_kind: Option<MemoryScope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub scope_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub states: Option<Vec<MemoryCandidateState>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub limit: Option<f64>,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct MemoryRecallSignal {
    pub target_ref: String,
    pub query_fingerprint: String,
    pub day: String,
    pub best_score: f64,
    pub hit_count: f64,
}

fn importance<'de, D: serde::Deserializer<'de>>(input: D) -> Result<u8, D::Error> {
    let number = f64::deserialize(input)?;
    if (1.0..=5.0).contains(&number) && number.fract() == 0. {
        Ok(number as u8)
    } else {
        Err(serde::de::Error::custom(
            "Memory importance must be 1, 2, 3, 4 or 5",
        ))
    }
}
