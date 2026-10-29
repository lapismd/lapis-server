use super::values::js_string;
use icu_collator::CollatorBorrowed;
use lapis_app_protocol::database::projections::queries::*;
use serde_json::{Map, Value};
use std::cmp::Ordering;

pub(super) fn scalar(value: &Value) -> Value {
    match value {
        Value::Array(_) | Value::Object(_) => Value::String(js_string(value)),
        _ => value.clone(),
    }
}
pub(super) fn compare(left: &Value, right: &Value, locale: &CollatorBorrowed<'_>) -> Ordering {
    match (left, right) {
        (Value::Null, Value::Null) => Ordering::Equal,
        (Value::Null, _) => Ordering::Greater,
        (_, Value::Null) => Ordering::Less,
        (Value::Number(a), Value::Number(b)) => a
            .as_f64()
            .unwrap()
            .partial_cmp(&b.as_f64().unwrap())
            .unwrap(),
        (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
        _ => locale.compare(&js_string(left), &js_string(right)),
    }
}
pub(super) fn matches(
    data: &Map<String, Value>,
    filter: &ProjectionFilter,
    locale: &CollatorBorrowed<'_>,
) -> bool {
    match filter {
        ProjectionFilter::And { operands } => operands.iter().all(|f| matches(data, f, locale)),
        ProjectionFilter::Or { operands } => operands.iter().any(|f| matches(data, f, locale)),
        ProjectionFilter::Not { operand } => !matches(data, operand, locale),
        ProjectionFilter::Exists { field } => !data.get(field).unwrap_or(&Value::Null).is_null(),
        ProjectionFilter::NotExists { field } => data.get(field).unwrap_or(&Value::Null).is_null(),
        ProjectionFilter::In { field, values } => values.iter().any(|v| {
            compare(
                data.get(field).unwrap_or(&Value::Null),
                &serde_json::to_value(v).expect("scalar"),
                locale,
            )
            .is_eq()
        }),
        ProjectionFilter::Compare {
            field,
            comparison,
            value,
        } => {
            let left = data.get(field).unwrap_or(&Value::Null);
            let value = serde_json::to_value(value).expect("scalar");
            let order = compare(left, &value, locale);
            match comparison {
                ProjectionComparison::Eq => order.is_eq(),
                ProjectionComparison::Ne => !order.is_eq(),
                ProjectionComparison::Lt => !left.is_null() && order.is_lt(),
                ProjectionComparison::Lte => !left.is_null() && !order.is_gt(),
                ProjectionComparison::Gt => !left.is_null() && order.is_gt(),
                ProjectionComparison::Gte => !left.is_null() && !order.is_lt(),
            }
        }
    }
}
pub(super) fn keys(
    left: &[Value],
    left_id: &str,
    right: &[Value],
    right_id: &str,
    order: &[ProjectionOrder],
    locale: &CollatorBorrowed<'_>,
) -> Ordering {
    for index in 0..left.len().max(right.len()).max(order.len()) {
        let a = left.get(index).unwrap_or(&Value::Null);
        let b = right.get(index).unwrap_or(&Value::Null);
        let mut comparison = compare(a, b, locale);
        if comparison.is_eq() {
            continue;
        }
        let spec = order.get(index);
        if let Some(nulls) = spec
            .and_then(|s| s.nulls)
            .filter(|_| a.is_null() || b.is_null())
        {
            return if a.is_null() == matches!(nulls, ProjectionNulls::First) {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }
        if matches!(
            spec.and_then(|s| s.direction),
            Some(ProjectionSortDirection::Desc)
        ) {
            comparison = comparison.reverse();
        }
        return comparison;
    }
    locale.compare(left_id, right_id)
}
