use super::{CaptureError, Result};
use serde_json::{Map, Value};

pub(crate) fn fail<T>(message: impl Into<String>) -> Result<T> {
    Err(CaptureError {
        name: "Error".into(),
        message: message.into(),
    })
}
pub(crate) fn record<'a>(value: &'a Value, label: &str) -> Result<&'a Map<String, Value>> {
    value.as_object().ok_or_else(|| CaptureError {
        name: "Error".into(),
        message: format!("{label} must be an object"),
    })
}
pub(crate) fn string<'a>(value: &'a Value, key: &str, label: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| CaptureError {
            name: "Error".into(),
            message: format!("{label}.{key} must be a non-empty string"),
        })
}
pub(crate) fn fields(value: &Value, keys: &[&str]) -> Result<()> {
    if value
        .as_object()
        .is_some_and(|o| o.keys().any(|k| !keys.contains(&k.as_str())))
    {
        return fail("Queued message contains unsupported fields");
    }
    Ok(())
}
pub(crate) fn utf16(value: &str) -> usize {
    value.encode_utf16().count()
}
pub(crate) fn bytes(value: &Value) -> usize {
    serde_json::to_vec(value).expect("JSON value").len()
}
pub(crate) fn safe_integer(value: &Value) -> Option<u64> {
    value
        .as_f64()
        .filter(|n| *n >= 0.0 && *n <= 9_007_199_254_740_991.0 && n.fract() == 0.0)
        .map(|n| n as u64)
}
pub(crate) fn thinking(value: Option<&Value>, label: &str) -> Result<()> {
    if value.is_some_and(|v| {
        !v.is_null()
            && !["default", "off", "low", "medium", "high"]
                .iter()
                .any(|s| v == s)
    }) {
        return fail(format!("{label} is invalid"));
    }
    Ok(())
}
pub(crate) fn timestamp(value: &Value, key: &str, label: &str) -> Result<()> {
    use chrono::{DateTime, Days, NaiveDate};
    let raw = string(value, key, label)?;
    let normalized = raw
        .get(..10)
        .and_then(|p| {
            NaiveDate::parse_from_str(&format!("{}-01", p.get(..7)?), "%Y-%m-%d")
                .ok()
                .and_then(|date| {
                    p.get(8..10)?
                        .parse::<u64>()
                        .ok()
                        .filter(|d| (1..=31).contains(d))
                        .and_then(|d| date.checked_add_days(Days::new(d - 1)))
                })
        })
        .map(|date| format!("{date}{}", &raw[10..]));
    let accepted = DateTime::parse_from_rfc3339(raw).is_ok()
        || DateTime::parse_from_rfc2822(raw).is_ok()
        || normalized.as_ref().is_some_and(|s| {
            DateTime::parse_from_rfc3339(s).is_ok()
                || NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok()
        });
    if !accepted {
        return fail(format!("{label}.{key} must be an ISO timestamp"));
    }
    Ok(())
}
pub(crate) fn one_of(value: &Value, values: &[&str]) -> bool {
    // Retained validators explicitly use String(...) for these enum checks.
    fn text(value: &Value) -> String {
        match value {
            Value::String(s) => s.clone(),
            Value::Array(a) => a
                .iter()
                .map(|v| if v.is_null() { String::new() } else { text(v) })
                .collect::<Vec<_>>()
                .join(","),
            Value::Object(_) => "[object Object]".into(),
            v => v.to_string(),
        }
    }
    values.contains(&text(value).as_str())
}
