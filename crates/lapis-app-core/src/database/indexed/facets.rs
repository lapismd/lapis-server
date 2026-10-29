use super::{Budget, filter_sql, limit, number, optional_text, slots, text};
use crate::database::{encoded, engine, invalid, reads::parse, store::Store};
use crate::services::Result;
use lapis_app_protocol::database::indexed::{queries::*, records::LinkRecord};
use std::collections::HashSet;

impl Store {
    pub async fn indexed_facets(&self, query: FacetQuery) -> Result<Vec<FacetRow>> {
        let count = limit(query.limit, 100, 1)?;
        let mut args = Vec::new();
        let prefix = filter_sql::prefixes("path", &query.path_prefixes, &mut args)
            .map(|s| format!(" AND {s}"))
            .unwrap_or_default();
        let sql = match query.kind {
            FacetKind::Tag => format!(
                "SELECT ancestor,count(DISTINCT path) FROM metadata_tag_ancestors WHERE 1=1 {prefix} GROUP BY ancestor ORDER BY count(DISTINCT path) DESC,ancestor LIMIT ?"
            ),
            FacetKind::PropertyName => format!(
                "SELECT min(name),count(DISTINCT path),group_concat(DISTINCT COALESCE(declared_type,inferred_type)) FROM metadata_properties WHERE 1=1 {prefix} GROUP BY normalized_name ORDER BY count(DISTINCT path) DESC,normalized_name LIMIT ?"
            ),
            FacetKind::PropertyPath => format!(
                "SELECT min(value),count(DISTINCT path),group_concat(DISTINCT metadata_type),max(top_level) FROM (SELECT property_path AS value,normalized_property_path AS normalized_value,value_type AS metadata_type,path,CASE WHEN property_name=normalized_property_path THEN 1 ELSE 0 END AS top_level FROM metadata_property_values UNION ALL SELECT name AS value,normalized_name AS normalized_value,COALESCE(declared_type,inferred_type) AS metadata_type,path,1 AS top_level FROM metadata_properties) property_paths WHERE 1=1 {prefix} GROUP BY normalized_value ORDER BY count(DISTINCT path) DESC,normalized_value LIMIT ?"
            ),
            FacetKind::PropertyValue => {
                let Some(name) = query.property_name.filter(|s| !s.is_empty()) else {
                    return Ok(vec![]);
                };
                let name = super::normalize::property_path(&name);
                args.splice(0..0, [text(name.clone()), text(name)]);
                format!(
                    "SELECT value_type,text_value,number_value,boolean_value,date_value,count(DISTINCT path) FROM metadata_property_values WHERE (property_name=? OR normalized_property_path=?) {prefix} GROUP BY value_type,text_value,number_value,boolean_value,date_value ORDER BY count(DISTINCT path) DESC LIMIT ?"
                )
            }
        };
        args.push(turso::Value::Integer(count as i64));
        let mut rows = self.connection.query(&sql, args).await.map_err(engine)?;
        let mut result = Vec::new();
        let mut budget = Budget::new();
        while let Some(row) = rows.next().await.map_err(engine)? {
            if result.len() >= 100_000 {
                return Err(invalid("Facet result exceeds 100,000 rows"));
            }
            let entry = match query.kind {
                FacetKind::PropertyValue => {
                    let kind = row.get::<String>(0).map_err(engine)?;
                    let (kind, value) = match kind.as_str() {
                        "null" => (ScalarKind::Null, MetadataScalar::Null),
                        "number" => (ScalarKind::Number, MetadataScalar::Number(number(&row, 2)?)),
                        "boolean" => (
                            ScalarKind::Boolean,
                            MetadataScalar::Boolean(row.get::<i64>(3).map_err(engine)? != 0),
                        ),
                        "date" => (
                            ScalarKind::Date,
                            MetadataScalar::String(
                                optional_text(&row, 4)?
                                    .or(optional_text(&row, 1)?)
                                    .unwrap_or_default(),
                            ),
                        ),
                        "string" => (
                            ScalarKind::String,
                            MetadataScalar::String(
                                optional_text(&row, 4)?
                                    .or(optional_text(&row, 1)?)
                                    .unwrap_or_default(),
                            ),
                        ),
                        _ => return Err(invalid("Invalid stored facet type")),
                    };
                    FacetRow {
                        value,
                        value_type: kind,
                        count: row_count(&row, 5)?,
                        metadata_types: None,
                        top_level: None,
                    }
                }
                kind => {
                    let value = row.get::<String>(0).map_err(engine)?;
                    let metadata_types = if matches!(kind, FacetKind::Tag) {
                        None
                    } else {
                        optional_text(&row, 2)?.map(|s| {
                            s.split(',')
                                .filter(|s| !s.is_empty())
                                .map(str::to_owned)
                                .collect()
                        })
                    };
                    FacetRow {
                        value: MetadataScalar::String(if matches!(kind, FacetKind::PropertyPath) {
                            replace_indices(&value)
                        } else {
                            value
                        }),
                        value_type: ScalarKind::String,
                        count: row_count(&row, 1)?,
                        metadata_types,
                        top_level: match kind {
                            FacetKind::PropertyName => Some(true),
                            FacetKind::PropertyPath => {
                                Some(row.get::<i64>(3).map_err(engine)? != 0)
                            }
                            _ => None,
                        },
                    }
                }
            };
            budget.take(encoded(&entry)?.len())?;
            result.push(entry);
        }
        encoded(&result)?;
        Ok(result)
    }
    pub async fn indexed_links(&self, query: LinkQuery) -> Result<Vec<LinkRecord>> {
        let mut paths = query.paths;
        if let Some(path) = query.path.filter(|s| !s.is_empty()) {
            paths.push(path);
        }
        let mut seen = HashSet::new();
        paths.retain(|p| seen.insert(p.clone()));
        if paths.is_empty() {
            return Ok(vec![]);
        }
        let field = match query.direction {
            LinkDirection::Incoming => "resolved_target_path",
            LinkDirection::Outgoing => "source_path",
        };
        let resolution = match query.resolution {
            Some(LinkResolution::Resolved) => " AND resolution_state='resolved'",
            Some(LinkResolution::Unresolved) => " AND resolution_state='unresolved'",
            _ => "",
        };
        let mut args: Vec<_> = paths.iter().cloned().map(text).collect();
        args.push(turso::Value::Integer(limit(query.limit, 1000, 0)? as i64));
        let mut rows=self.connection.query(&format!("SELECT data_json FROM metadata_links WHERE {field} IN ({}){resolution} ORDER BY source_path,ordinal LIMIT ?",slots(paths.len())),args).await.map_err(engine)?;
        let mut result = Vec::new();
        let mut budget = Budget::new();
        while let Some(row) = rows.next().await.map_err(engine)? {
            if result.len() >= 100_000 {
                return Err(invalid("Link result exceeds 100,000 rows"));
            }
            let json = row.get::<String>(0).map_err(engine)?;
            budget.take(json.len())?;
            result.push(
                serde_json::from_value(parse(json)?)
                    .map_err(|_| invalid("Invalid stored metadata link"))?,
            );
        }
        encoded(&result)?;
        Ok(result)
    }
}
fn row_count(row: &turso::Row, column: usize) -> Result<u64> {
    u64::try_from(row.get::<i64>(column).map_err(engine)?)
        .map_err(|_| invalid("Invalid facet count"))
}
fn replace_indices(value: &str) -> String {
    let mut out = String::new();
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '[' {
            out.push(c);
            continue;
        }
        let mut digits = String::new();
        while chars.peek().is_some_and(char::is_ascii_digit) {
            digits.push(chars.next().unwrap());
        }
        if !digits.is_empty() && chars.peek() == Some(&']') {
            out.push_str("[]");
            chars.next();
        } else {
            out.push('[');
            out.push_str(&digits);
        }
    }
    out
}
