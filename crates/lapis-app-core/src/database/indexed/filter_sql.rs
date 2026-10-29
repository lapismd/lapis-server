use super::{normalize, text};
use lapis_app_protocol::database::indexed::queries::*;
use turso::Value;

pub(super) fn paths(
    query: &MetadataQuery,
    after: Option<&str>,
    limit: usize,
) -> (String, Vec<Value>) {
    let mut args = Vec::new();
    let mut clauses = vec!["f.indexed=1".into(), "f.deleted=0".into()];
    if let Some(after) = after.filter(|s| !s.is_empty()) {
        clauses.push("f.path>?".into());
        args.push(text(after));
    }
    if !query.extensions.is_empty() {
        clauses.push(format!(
            "f.extension IN ({})",
            super::slots(query.extensions.len())
        ));
        args.extend(
            query
                .extensions
                .iter()
                .map(|s| text(normalize::extension(s))),
        );
    }
    if let Some(prefix) = prefixes("f.path", &query.path_prefixes, &mut args) {
        clauses.push(prefix);
    }
    if query.exclude_hidden_paths {
        clauses.push("f.path NOT GLOB '.*' AND f.path NOT GLOB '*/.*'".into());
    }
    for tag in &query.required_tags {
        clauses.push("EXISTS(SELECT 1 FROM metadata_tag_ancestors ta WHERE ta.path=f.path AND ta.ancestor=?)".into());
        args.push(text(normalize::tag(tag)));
    }
    for target in &query.resolved_target_paths {
        clauses.push("EXISTS(SELECT 1 FROM metadata_links ml WHERE ml.source_path=f.path AND ml.resolved_target_path=?)".into());
        args.push(text(target.as_str()));
    }
    for filter in &query.property_filters {
        clauses.push(property(filter, &mut args));
    }
    let mut order = Vec::new();
    for sort in &query.sort {
        let direction = match sort.direction {
            SortDirection::Asc => "ASC",
            SortDirection::Desc => "DESC",
        };
        order.push(match &sort.field {
            MetadataSortField::File{field}=>format!("f.{} {direction}",match field {FileSortField::Path=>"path",FileSortField::Extension=>"extension",FileSortField::Mtime=>"mtime",FileSortField::Size=>"size"}),
            MetadataSortField::Property{name}=>{args.push(text(name.to_lowercase()));format!("(SELECT COALESCE(pv.number_value,pv.boolean_value,pv.date_value,pv.text_value) FROM metadata_property_values pv WHERE pv.path=f.path AND pv.property_name=? ORDER BY pv.property_ordinal,pv.value_ordinal LIMIT 1) {direction}")}
        });
    }
    order.push("f.path ASC".into());
    args.push(Value::Integer(limit as i64));
    (
        format!(
            "SELECT f.path FROM files f WHERE {} ORDER BY {} LIMIT ?",
            clauses.join(" AND "),
            order.join(",")
        ),
        args,
    )
}
pub(super) fn prefixes(column: &str, prefixes: &[String], args: &mut Vec<Value>) -> Option<String> {
    if prefixes.is_empty() {
        return None;
    }
    for raw in prefixes {
        let prefix = raw.trim_matches('/');
        args.push(text(prefix));
        args.push(text(format!(
            "{}/%",
            prefix.replace('%', "\\%").replace('_', "\\_")
        )));
    }
    Some(format!(
        "EXISTS(SELECT 1 FROM (VALUES {}) AS requested_prefixes WHERE {column}=requested_prefixes.column1 OR {column} LIKE requested_prefixes.column2 ESCAPE '\\')",
        vec!["(?,?)"; prefixes.len()].join(",")
    ))
}

fn property(filter: &PropertyFilter, args: &mut Vec<Value>) -> String {
    let name = normalize::property_path(&filter.name);
    args.extend([text(name.clone()), text(name)]);
    if matches!(filter.op, PropertyOp::Exists | PropertyOp::NotExists) {
        return format!(
            "{}(EXISTS(SELECT 1 FROM metadata_properties mp WHERE mp.path=f.path AND mp.normalized_name=?) OR EXISTS(SELECT 1 FROM metadata_property_values pv WHERE pv.path=f.path AND pv.normalized_property_path=?))",
            if matches!(filter.op, PropertyOp::NotExists) {
                "NOT "
            } else {
                ""
            }
        );
    }
    let (column, value) = match &filter.value {
        Some(MetadataScalar::Number(n)) => ("number_value", Value::Real(*n)),
        Some(MetadataScalar::Boolean(b)) => ("boolean_value", Value::Integer(i64::from(*b))),
        Some(MetadataScalar::String(s)) if normalize::date(s) => ("date_value", text(s.as_str())),
        Some(MetadataScalar::String(s)) => ("text_value", text(s.as_str())),
        _ => ("text_value", Value::Null),
    };
    args.push(value);
    let op = match filter.op {
        PropertyOp::Eq => "=",
        PropertyOp::Ne => "!=",
        PropertyOp::Gt => ">",
        PropertyOp::Ge => ">=",
        PropertyOp::Lt => "<",
        PropertyOp::Le => "<=",
        _ => unreachable!("existence operators handled above"),
    };
    format!(
        "EXISTS(SELECT 1 FROM metadata_property_values pv WHERE pv.path=f.path AND (pv.property_name=? OR pv.normalized_property_path=?) AND pv.{column} {op} ?)"
    )
}
