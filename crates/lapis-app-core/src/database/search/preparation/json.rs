use serde_json::Value;

/// JSON.stringify-compatible number and object-key order for metadata text.
pub(in crate::database::search) fn stringify(value: &Value) -> String {
    match value {
        Value::Array(items) => format!(
            "[{}]",
            items.iter().map(stringify).collect::<Vec<_>>().join(",")
        ),
        Value::Object(fields) => {
            let mut entries: Vec<_> = fields.iter().collect();
            entries.sort_by_key(|(key, _)| array_index(key).map_or((1, 0), |i| (0, i)));
            format!(
                "{{{}}}",
                entries
                    .into_iter()
                    .map(|(key, value)| {
                        format!(
                            "{}:{}",
                            serde_json::to_string(key).expect("JSON key"),
                            stringify(value)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
        Value::Number(number) => {
            let number = number.as_f64().expect("JSON number");
            if number == 0. {
                "0".into()
            } else if (1e-6..1e21).contains(&number.abs()) {
                number.to_string()
            } else {
                let formatted = format!("{number:e}");
                let (mantissa, exponent) = formatted.split_once('e').expect("exponential number");
                format!(
                    "{mantissa}e{:+}",
                    exponent.parse::<i32>().expect("exponent")
                )
            }
        }
        _ => serde_json::to_string(value).expect("JSON value"),
    }
}
fn array_index(key: &str) -> Option<u32> {
    key.parse::<u32>()
        .ok()
        .filter(|n| *n < u32::MAX && n.to_string() == key)
}
