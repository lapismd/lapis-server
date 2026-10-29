use crate::database::engine;
use crate::services::Result;
use lapis_app_protocol::database::projections::*;
use serde_json::Value;

pub(super) async fn write(
    connection: &turso::Connection,
    id: &str,
    row: &ProjectionRow,
    definition: &ProjectionDefinition,
) -> Result<()> {
    let mut fields = definition
        .fields
        .iter()
        .map(|(key, value)| (key.as_str(), value))
        .collect::<Vec<_>>();
    // Object.entries enumerates array-index keys first, then insertion order.
    fields.sort_by_key(|(key, _)| array_index(key).map_or((1, 0), |n| (0, n)));
    for (field, definition) in fields
        .into_iter()
        .filter(|(_, d)| d.indexed || d.sortable)
        .take(64)
    {
        let raw = row.data.get(field).unwrap_or(&Value::Null);
        let items = if matches!(definition.cardinality, Some(ProjectionCardinality::Many)) {
            raw.as_array()
                .map(Vec::as_slice)
                .unwrap_or(std::slice::from_ref(raw))
        } else {
            std::slice::from_ref(raw)
        };
        for (ordinal, item) in items.iter().enumerate() {
            let mut columns = vec![turso::Value::Null; 5];
            if !item.is_null() {
                let text = js_string(item);
                match definition.r#type {
                    ProjectionFieldType::Number => columns[1] = numeric(item, &text),
                    ProjectionFieldType::Datetime => columns[4] = numeric(item, &text),
                    ProjectionFieldType::Boolean => {
                        columns[2] = turso::Value::Integer(i64::from(match item {
                            Value::Bool(b) => *b,
                            Value::Number(n) => n.as_f64().is_some_and(|v| v != 0.0),
                            _ => !text.is_empty(),
                        }))
                    }
                    ProjectionFieldType::Date => columns[3] = bounded(text),
                    _ => columns[0] = bounded(text),
                }
            }
            let kind = match definition.r#type {
                ProjectionFieldType::String => "string",
                ProjectionFieldType::Number => "number",
                ProjectionFieldType::Boolean => "boolean",
                ProjectionFieldType::Date => "date",
                ProjectionFieldType::Datetime => "datetime",
                ProjectionFieldType::Path => "path",
                ProjectionFieldType::Json => "json",
            };
            let mut args = vec![
                turso::Value::Text(id.into()),
                turso::Value::Text(row.id.clone()),
                turso::Value::Text(field.into()),
                turso::Value::Integer(ordinal as i64),
                turso::Value::Text(kind.into()),
            ];
            args.extend(columns);
            connection.execute("INSERT INTO index_projection_values(projection_id,row_id,field,ordinal,value_type,text_value,number_value,boolean_value,date_value,datetime_value) VALUES(?,?,?,?,?,?,?,?,?,?)",args).await.map_err(engine)?;
        }
    }
    Ok(())
}
fn array_index(key: &str) -> Option<u32> {
    key.parse::<u32>()
        .ok()
        .filter(|n| *n < u32::MAX && n.to_string() == key)
}
fn bounded(value: String) -> turso::Value {
    turso::Value::Text(String::from_utf16_lossy(
        &value.encode_utf16().take(4096).collect::<Vec<_>>(),
    ))
}
fn numeric(value: &Value, text: &str) -> turso::Value {
    if let Value::Bool(b) = value {
        turso::Value::Real(f64::from(*b))
    } else {
        number(text)
    }
}
pub(in crate::database) fn number(value: &str) -> turso::Value {
    let value = value.trim_matches(|c: char| {
        matches!(c,
            '\u{0009}'..='\u{000d}' | '\u{0020}' | '\u{00a0}' | '\u{1680}' |
            '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' |
            '\u{205f}' | '\u{3000}' | '\u{feff}'
        )
    });
    let parsed = if value.is_empty() {
        Some(0.0)
    } else if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        radix(hex, 16)
    } else if let Some(binary) = value
        .strip_prefix("0b")
        .or_else(|| value.strip_prefix("0B"))
    {
        radix(binary, 2)
    } else if let Some(octal) = value
        .strip_prefix("0o")
        .or_else(|| value.strip_prefix("0O"))
    {
        radix(octal, 8)
    } else {
        value.parse::<f64>().ok()
    };
    parsed
        .filter(|n| n.is_finite())
        .map_or(turso::Value::Null, turso::Value::Real)
}
fn radix(s: &str, base: u32) -> Option<f64> {
    if s.is_empty() {
        return None;
    }
    s.chars().try_fold(0.0, |n, c| {
        c.to_digit(base).map(|v| n * f64::from(base) + f64::from(v))
    })
}
pub(in crate::database) fn js_string(value: &Value) -> String {
    match value {
        Value::Null => "null".into(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => {
            let n = n.as_f64().expect("JSON number");
            if n == 0.0 {
                "0".into()
            } else if (1e-6..1e21).contains(&n.abs()) {
                n.to_string()
            } else {
                let s = format!("{n:e}");
                if let Some((mantissa, exp)) = s.split_once('e') {
                    let exponent = exp.parse::<i32>().expect("formatted number");
                    format!("{mantissa}e{exponent:+}")
                } else {
                    s
                }
            }
        }
        Value::Object(_) => "[object Object]".into(),
        Value::Array(values) => values
            .iter()
            .map(|v| {
                if v.is_null() {
                    String::new()
                } else {
                    js_string(v)
                }
            })
            .collect::<Vec<_>>()
            .join(","),
    }
}
