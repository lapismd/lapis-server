use super::records::{FileRecord, LinkRecord, MetadataRecord, PropertyRecord, TagRecord};
use crate::database::tasks::record::TaskRecord;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct IndexedFile {
    pub file: FileRecord,
    pub metadata: MetadataRecord,
    pub links: Vec<LinkRecord>,
    pub tags: Vec<TagRecord>,
    pub properties: Vec<PropertyRecord>,
    #[serde(
        default,
        deserialize_with = "present_task",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(type = "TaskRecord | null", optional)]
    pub task: Option<Option<TaskRecord>>,
}
fn present_task<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Option<TaskRecord>>, D::Error> {
    Option::<TaskRecord>::deserialize(deserializer).map(Some)
}
