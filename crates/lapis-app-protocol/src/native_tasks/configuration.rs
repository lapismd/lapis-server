use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct NativeTaskDayParts {
    pub afternoon_starts_at: String,
    pub evening_starts_at: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NativeTaskDailySettings {
    pub folder: String,
    pub date_format: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct NativeTaskDailyFormat {
    pub locale: String,
    pub first_day: u32,
    pub minimal_days: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[ts(tag = "kind", rename_all = "snake_case")]
pub enum NativeTaskProvider {
    Default {},
    Unavailable { message: String },
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct NativeTaskConfiguration {
    pub day_parts: NativeTaskDayParts,
    pub daily: NativeTaskDailySettings,
    pub format: NativeTaskDailyFormat,
    pub provider: NativeTaskProvider,
}
