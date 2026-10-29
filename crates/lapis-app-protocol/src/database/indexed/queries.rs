use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(untagged)]
#[ts(untagged)]
pub enum MetadataScalar {
    Null,
    Boolean(bool),
    Number(f64),
    String(String),
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, TS)]
pub enum PropertyOp {
    #[serde(rename = "exists")]
    #[ts(rename = "exists")]
    Exists,
    #[serde(rename = "not-exists")]
    #[ts(rename = "not-exists")]
    NotExists,
    #[serde(rename = "=")]
    #[ts(rename = "=")]
    Eq,
    #[serde(rename = "!=")]
    #[ts(rename = "!=")]
    Ne,
    #[serde(rename = ">")]
    #[ts(rename = ">")]
    Gt,
    #[serde(rename = ">=")]
    #[ts(rename = ">=")]
    Ge,
    #[serde(rename = "<")]
    #[ts(rename = "<")]
    Lt,
    #[serde(rename = "<=")]
    #[ts(rename = "<=")]
    Le,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct PropertyFilter {
    pub name: String,
    pub op: PropertyOp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub value: Option<MetadataScalar>,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename_all = "lowercase")]
pub enum FileSortField {
    Path,
    Extension,
    Mtime,
    Size,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
#[ts(tag = "kind", rename_all = "lowercase")]
pub enum MetadataSortField {
    File { field: FileSortField },
    Property { name: String },
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "UPPERCASE")]
#[ts(rename_all = "UPPERCASE")]
pub enum SortDirection {
    Asc,
    Desc,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct MetadataSort {
    pub field: MetadataSortField,
    pub direction: SortDirection,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct MetadataQuery {
    #[serde(default)]
    #[ts(as = "Option<Vec<String>>", optional)]
    pub extensions: Vec<String>,
    #[serde(default)]
    #[ts(as = "Option<Vec<String>>", optional)]
    pub path_prefixes: Vec<String>,
    #[serde(default)]
    #[ts(as = "Option<bool>", optional)]
    pub exclude_hidden_paths: bool,
    #[serde(default)]
    #[ts(as = "Option<Vec<PropertyFilter>>", optional)]
    pub property_filters: Vec<PropertyFilter>,
    #[serde(default)]
    #[ts(as = "Option<Vec<String>>", optional)]
    pub required_tags: Vec<String>,
    #[serde(default)]
    #[ts(as = "Option<Vec<String>>", optional)]
    pub resolved_target_paths: Vec<String>,
    #[serde(default)]
    #[ts(as = "Option<Vec<MetadataSort>>", optional)]
    pub sort: Vec<MetadataSort>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub limit: Option<usize>,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct ManifestQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub limit: Option<usize>,
    #[serde(default)]
    #[ts(as = "Option<Vec<String>>", optional)]
    pub paths: Vec<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename_all = "lowercase")]
pub enum MetadataDomain {
    Metadata,
    Properties,
    Tags,
    Links,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct MetadataPageQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub query: Option<MetadataQuery>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub after: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub limit: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub include: Option<Vec<MetadataDomain>>,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "kebab-case")]
#[ts(rename_all = "kebab-case")]
pub enum FacetKind {
    Tag,
    PropertyName,
    PropertyPath,
    PropertyValue,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct FacetQuery {
    pub kind: FacetKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub property_name: Option<String>,
    #[serde(default)]
    #[ts(as = "Option<Vec<String>>", optional)]
    pub path_prefixes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub limit: Option<usize>,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename_all = "lowercase")]
pub enum ScalarKind {
    Null,
    String,
    Number,
    Boolean,
    Date,
}
#[derive(Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct FacetRow {
    pub value: MetadataScalar,
    pub value_type: ScalarKind,
    #[ts(type = "number")]
    pub count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub metadata_types: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub top_level: Option<bool>,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename_all = "lowercase")]
pub enum LinkDirection {
    Incoming,
    Outgoing,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename_all = "lowercase")]
pub enum LinkResolution {
    Resolved,
    Unresolved,
    All,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct LinkQuery {
    pub direction: LinkDirection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub path: Option<String>,
    #[serde(default)]
    #[ts(as = "Option<Vec<String>>", optional)]
    pub paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub resolution: Option<LinkResolution>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub limit: Option<usize>,
}
