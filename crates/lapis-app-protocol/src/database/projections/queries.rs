use crate::database::indexed::queries::MetadataScalar;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename_all = "lowercase")]
pub enum ProjectionComparison {
    Eq,
    Ne,
    Lt,
    Lte,
    Gt,
    Gte,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
#[ts(tag = "op", rename_all = "kebab-case")]
pub enum ProjectionFilter {
    And {
        operands: Vec<Self>,
    },
    Or {
        operands: Vec<Self>,
    },
    Not {
        operand: Box<Self>,
    },
    Compare {
        field: String,
        comparison: ProjectionComparison,
        value: MetadataScalar,
    },
    In {
        field: String,
        values: Vec<MetadataScalar>,
    },
    Exists {
        field: String,
    },
    NotExists {
        field: String,
    },
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename_all = "lowercase")]
pub enum ProjectionSortDirection {
    Asc,
    Desc,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename_all = "lowercase")]
pub enum ProjectionNulls {
    First,
    Last,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(deny_unknown_fields)]
pub struct ProjectionOrder {
    pub field: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub direction: Option<ProjectionSortDirection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub nulls: Option<ProjectionNulls>,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct ProjectionQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub r#where: Option<ProjectionFilter>,
    #[serde(default)]
    #[ts(as = "Option<Vec<String>>", optional)]
    pub select: Vec<String>,
    #[serde(default)]
    #[ts(as = "Option<Vec<ProjectionOrder>>", optional)]
    pub order_by: Vec<ProjectionOrder>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub limit: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub after: Option<String>,
    #[serde(default)]
    #[ts(as = "Option<bool>", optional)]
    pub include_stale: bool,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename_all = "lowercase")]
pub enum ProjectionRelationDirection {
    In,
    Out,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct ProjectionRelatedQuery {
    pub projection_id: String,
    pub row_id: String,
    pub relation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub direction: Option<ProjectionRelationDirection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub target_where: Option<ProjectionFilter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub limit: Option<f64>,
}
#[derive(Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename_all = "lowercase")]
pub enum ProjectionIndexStatus {
    Ready,
    Building,
    Error,
}
#[derive(Debug, Deserialize, Serialize, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(rename_all = "camelCase")]
pub struct ProjectionQueryResult {
    #[ts(type = "Array<{ [key: string]: unknown }>")]
    pub rows: Vec<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub next_cursor: Option<String>,
    #[ts(type = "number")]
    pub revision: u64,
    pub complete: bool,
    pub index_status: ProjectionIndexStatus,
}
