use regress::Regex;
use serde_json::Value;
use std::sync::LazyLock;

static TAGS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(#[/a-zA-Z0-9_-]+\s*,\s*)*(#[/a-zA-Z0-9_-]+\s*)?$").unwrap());
static DATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d{4}-(0[1-9]|1[012])-(0[1-9]|[12][0-9]|3[01])$").unwrap());
static DATETIME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^([0-9]{4})-?(1[0-2]|0[1-9])-?(3[01]|0[1-9]|[12][0-9])[T ](2[0-3]|[01][0-9]):?([0-5][0-9])(:?([0-5][0-9]))?$").unwrap()
});

pub(super) fn inferred(name: &str, value: &Value) -> &'static str {
    match name.trim().to_lowercase().as_str() {
        "tags" | "tag" => return "tags",
        "alias" | "aliases" => return "aliases",
        _ => {}
    }
    match value {
        Value::Number(_) => "number",
        Value::Bool(_) => "checkbox",
        Value::Null => "text",
        Value::Object(_) => "object",
        Value::Array(values) => {
            if values
                .iter()
                .all(|value| value.is_string() || value.is_number())
            {
                "multitext"
            } else {
                "array"
            }
        }
        Value::String(value) => {
            let trimmed = value.trim();
            if matches!(trimmed.to_lowercase().as_str(), "true" | "false") {
                "checkbox"
            } else if TAGS.find(value).is_some() {
                "tags"
            } else if DATE.find(trimmed).is_some() {
                "date"
            } else if DATETIME.find(trimmed).is_some() {
                "datetime"
            } else if !trimmed.is_empty() && trimmed.bytes().all(|c| c.is_ascii_digit()) {
                "number"
            } else {
                "text"
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn retained_application_type_inference_fixtures() {
        let fixtures: Vec<serde_json::Value> = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/native-metadata-types.json"
        )))
        .unwrap();
        for fixture in fixtures {
            assert_eq!(
                super::inferred(fixture["name"].as_str().unwrap(), &fixture["value"]),
                fixture["expected"].as_str().unwrap(),
                "{fixture:?}"
            );
        }
    }
}
