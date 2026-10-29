use lapis_app_protocol::turns::TurnInput;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// One explicit authored message. Captured tags never select a native target.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParticipantMessageInput {
    pub acceptance_id: Uuid,
    pub user: Value,
    pub dispatch: Value,
    pub turns: Vec<TurnInput>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profiles: Option<Vec<lapis_app_protocol::participants::ParticipantProfile>>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticipantState {
    Pending,
    Started,
    Completed,
    Rejected,
    Unknown,
    NotSent,
    Cancelled,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParticipantDelivery {
    pub delivery_id: String,
    pub request_id: Uuid,
    pub agent_binding_id: Uuid,
    pub session_id: Uuid,
    pub state: ParticipantState,
    pub outcome_published: bool,
}
impl ParticipantState {
    pub(super) fn terminal(self) -> bool {
        !matches!(self, Self::Pending | Self::Started)
    }
    pub(super) fn outcome(phase: lapis_app_protocol::turns::TurnPhase) -> Self {
        use lapis_app_protocol::turns::TurnPhase;
        match phase {
            TurnPhase::Completed => Self::Completed,
            TurnPhase::Failed => Self::Rejected,
            _ => Self::Unknown,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParticipantMessage {
    pub schema_version: u32,
    pub vault_id: Uuid,
    pub acceptance_id: Uuid,
    pub conversation_id: Uuid,
    pub input: ParticipantMessageInput,
    pub deliveries: Vec<ParticipantDelivery>,
    pub recovered: bool,
}
