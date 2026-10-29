use super::query::{Expr, Kind};
use crate::database::projections::values::{js_string, number};
use icu_collator::CollatorBorrowed;
use lapis_app_protocol::database::indexed::records::PropertyRecord;
use serde_json::Value;

pub(super) struct Properties(pub Vec<(String, Value)>);
impl Properties {
    pub fn new(indexed: Vec<PropertyRecord>, metadata: Option<&str>) -> Self {
        let entries = indexed
            .into_iter()
            .map(|p| (p.name, p.value))
            .collect::<Vec<_>>();
        let Some(Value::Object(metadata)) = metadata.and_then(|s| serde_json::from_str(s).ok())
        else {
            return Self(entries);
        };
        let mut result = Self(vec![]);
        for (name, value) in entries.into_iter().chain(metadata) {
            result.set(name, value)
        }
        result
    }
    fn set(&mut self, name: String, value: Value) {
        if let Some(entry) = self
            .0
            .iter_mut()
            .find(|(key, _)| key.to_lowercase() == name.to_lowercase())
        {
            *entry = (name, value)
        } else {
            self.0.push((name, value))
        }
    }
    pub fn has_required(&self, names: &[String]) -> bool {
        names.iter().all(|n| {
            self.0.iter().any(|(key, _)| {
                key.to_lowercase() == *n
                    || key.to_lowercase() == n.replace("[]", "").split('.').next().unwrap_or(n)
            })
        })
    }
    pub fn get(&self, name: &str) -> Option<Value> {
        if let Some((_, value)) = self
            .0
            .iter()
            .find(|(key, _)| key.to_lowercase() == name.to_lowercase())
        {
            return Some(value.clone());
        }
        let clean = name.replace("[]", "");
        let parts = clean
            .split('.')
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>();
        if parts.len() < 2 {
            return None;
        }
        let (_, root) = self
            .0
            .iter()
            .find(|(key, _)| key.to_lowercase() == parts[0].to_lowercase())?;
        let mut values = vec![];
        nested(root, &parts[1..], &mut values);
        if values.len() == 1 {
            values.pop()
        } else if values.is_empty() {
            None
        } else {
            Some(Value::Array(values))
        }
    }
}
fn nested(value: &Value, parts: &[&str], out: &mut Vec<Value>) {
    if parts.is_empty() {
        out.push(value.clone())
    } else if let Value::Array(items) = value {
        for item in items {
            nested(item, parts, out)
        }
    } else if let Some(value) = value.as_object().and_then(|o| o.get(parts[0])) {
        nested(value, &parts[1..], out)
    }
}
pub(super) fn nullish(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(s) => s.trim_matches(super::text::whitespace).is_empty(),
        Value::Array(items) => items.is_empty() || items.iter().any(nullish),
        _ => false,
    }
}
pub(super) fn texts(value: &Value) -> Vec<String> {
    match value {
        Value::Array(v) => v.iter().flat_map(texts).collect(),
        Value::Object(_) => vec![super::super::preparation::json::stringify(value)],
        Value::Null => vec![String::new()],
        _ => vec![js_string(value)],
    }
}
fn numeric(value: &Value) -> Option<f64> {
    match value {
        Value::Null => Some(0.),
        Value::Bool(b) => Some(f64::from(*b)),
        _ => match number(&js_string(value)) {
            turso::Value::Real(v) => Some(v),
            _ => None,
        },
    }
}
fn date(text: &str) -> Option<f64> {
    if let Ok(d) = chrono::DateTime::parse_from_rfc3339(text) {
        return Some(d.timestamp_millis() as f64);
    }
    if let Ok(d) = chrono::DateTime::parse_from_rfc2822(text) {
        return Some(d.timestamp_millis() as f64);
    }
    chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|d| d.and_utc().timestamp_millis() as f64)
}
pub(super) fn compare(
    value: &Value,
    op: &str,
    target: &Expr,
    locale: &CollatorBorrowed<'_>,
) -> bool {
    if let Value::Array(v) = value {
        return v.iter().any(|v| compare(v, op, target, locale));
    }
    let Expr::Literal(l) = target else {
        return false;
    };
    if matches!(l.kind, Kind::Null) {
        return false;
    }
    let right = Value::String(l.text.clone());
    let order = if let (Some(a), Some(b)) = (numeric(value), numeric(&right)) {
        a.partial_cmp(&b).unwrap()
    } else {
        let a = js_string(value);
        let b = js_string(&right);
        if let (Some(a), Some(b)) = (date(&a), date(&b)) {
            a.partial_cmp(&b).unwrap()
        } else {
            locale.compare(&a, &b)
        }
    };
    match op {
        "<" => order.is_lt(),
        "<=" => !order.is_gt(),
        ">" => order.is_gt(),
        ">=" => !order.is_lt(),
        _ => false,
    }
}
