//! Retained optional request/skip diagnostics; no model execution authority.
use super::retrieval::SearchMode;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(rename_all = "kebab-case")]
pub enum SearchEnhancementRuntime {
    WebPwa,
    DenoDesktop,
    TauriDesktop,
    NodeServer,
    DenoServer,
    Test,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(rename_all = "kebab-case")]
pub enum SearchEnhancementProviderKind {
    LocalModel,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct SearchEnhancementProvider {
    pub kind: SearchEnhancementProviderKind,
    pub query_expansion_model_id: String,
    pub reranker_model_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub allow_remote_models: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub local_model_path: Option<String>,
    pub max_expanded_queries: f64,
    pub max_rerank_candidates: f64,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct SearchEnhancementRequest {
    pub runtime_target: SearchEnhancementRuntime,
    pub provider: SearchEnhancementProvider,
    pub expand_query: bool,
    pub rerank_results: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct SearchEnhancementSignal {
    pub requested_mode: SearchMode,
    pub top_lexical_score: f64,
    pub top_vector_score: f64,
    pub top_fused_score: f64,
    pub result_count: usize,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(rename_all = "kebab-case")]
pub enum SearchEnhancementSkipReason {
    UnsupportedRuntime,
    StrongSignal,
    ProviderUnavailable,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct SearchEnhancementDiagnostics {
    pub runtime_target: SearchEnhancementRuntime,
    pub provider_kind: SearchEnhancementProviderKind,
    pub expand_query: bool,
    pub rerank_results: bool,
    pub applied: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub skip_reason: Option<SearchEnhancementSkipReason>,
    pub signal: SearchEnhancementSignal,
}
