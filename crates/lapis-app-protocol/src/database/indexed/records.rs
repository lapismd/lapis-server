use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct FileRecord {
    pub path: String,
    pub normalized_path: String,
    pub extension: String,
    pub mtime: f64,
    pub size: f64,
    pub hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub parser_version: Option<String>,
    pub indexed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub deleted: Option<bool>,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct MetadataRecord {
    pub path: String,
    pub hash: String,
    pub parser_version: String,
    #[ts(type = "unknown")]
    pub metadata: Value,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename_all = "lowercase")]
pub enum LinkType {
    Link,
    Embed,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(rename_all = "kebab-case")]
pub enum LinkKind {
    Reference,
    TaskEntry,
    ListEntry,
    NavigationItem,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct LinkRecord {
    pub source_path: String,
    pub target_text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub original: Option<String>,
    pub resolved_target_path: Option<String>,
    #[serde(rename = "type")]
    #[ts(rename = "type")]
    pub link_type: LinkType,
    #[serde(
        default,
        deserialize_with = "super::super::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(type = "unknown", optional)]
    pub position: Option<Value>,
    pub count: f64,
    #[serde(
        default,
        deserialize_with = "nullable_text",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(type = "string | null", optional)]
    pub heading: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub kind: Option<LinkKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub ordinal: Option<f64>,
}
fn nullable_text<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<Option<String>>, D::Error> {
    Option::<String>::deserialize(d).map(Some)
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct TagRecord {
    pub path: String,
    pub tag: String,
    pub parts: Vec<String>,
    pub hierarchy: Vec<String>,
    #[serde(
        default,
        deserialize_with = "super::super::present",
        skip_serializing_if = "Option::is_none"
    )]
    #[ts(type = "unknown", optional)]
    pub position: Option<Value>,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct PropertyRecord {
    pub path: String,
    pub name: String,
    pub inferred_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub declared_type: Option<String>,
    #[ts(type = "unknown")]
    pub value: Value,
}
/// Core publication is independent of optional plugin projection work.
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct CoreIndexedFile {
    pub file: FileRecord,
    pub metadata: MetadataRecord,
    pub links: Vec<LinkRecord>,
    pub tags: Vec<TagRecord>,
    pub properties: Vec<PropertyRecord>,
}
#[derive(Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct IndexedRow {
    pub file: FileRecord,
    pub metadata: Option<MetadataRecord>,
    pub properties: Vec<PropertyRecord>,
    pub tags: Vec<TagRecord>,
    pub links: Vec<LinkRecord>,
}
#[derive(Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct ManifestPage {
    pub rows: Vec<FileRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub next_cursor: Option<String>,
}
#[derive(Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct MetadataPage {
    pub rows: Vec<IndexedRow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub next_cursor: Option<String>,
}
