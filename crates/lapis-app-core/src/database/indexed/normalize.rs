use serde_json::Value;

pub(super) fn trim(value: &str) -> &str {
    crate::text::trim(value)
}
pub(super) fn extension(value: &str) -> String {
    let s = trim(value);
    s.strip_prefix('.').unwrap_or(s).to_lowercase()
}
pub(super) fn tag(value: &str) -> String {
    let s = trim(value);
    s.strip_prefix('#').unwrap_or(s).to_lowercase()
}
pub(super) fn date(value: &str) -> bool {
    let b = value.as_bytes();
    b.len() >= 10
        && [0, 1, 2, 3, 5, 6, 8, 9]
            .into_iter()
            .all(|i| b[i].is_ascii_digit())
        && b[4] == b'-'
        && b[7] == b'-'
        && (b.len() == 10 || b[10] == b'T')
}
pub(super) fn property_path(value: &str) -> String {
    let lower = value.to_lowercase();
    let mut chars = lower.chars().peekable();
    let mut out = String::new();
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
pub(super) fn flatten<'a>(value: &'a Value, path: String, out: &mut Vec<(String, &'a Value)>) {
    match value {
        Value::Array(values) => {
            for (i, v) in values.iter().enumerate() {
                flatten(v, format!("{path}[{i}]"), out);
            }
        }
        Value::Object(values) => {
            for (key, v) in values {
                flatten(
                    v,
                    if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{path}.{key}")
                    },
                    out,
                );
            }
        }
        _ => out.push((path, value)),
    }
}
