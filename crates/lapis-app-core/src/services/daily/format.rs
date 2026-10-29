use super::{DailyDocumentError, tokens};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Captured source context is explicit; host Intl defaults do not select policy.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DailyFormatContext {
    pub locale: String,
    pub first_day: u32,
    pub minimal_days: u32,
}
impl DailyFormatContext {
    pub(super) fn validate(&self) -> Result<(), DailyDocumentError> {
        if !matches!(
            (self.locale.as_str(), self.first_day, self.minimal_days),
            ("en-US", 7, 4) | ("en-US", 7, 1) | ("en-GB", 1, 4)
        ) {
            return Err(DailyDocumentError::new(
                "Native daily formatting context is not qualified.",
            ));
        }
        Ok(())
    }
}
pub(super) fn date(value: &str) -> Result<NaiveDate, DailyDocumentError> {
    let bytes = value.as_bytes();
    let valid = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && [0, 1, 2, 3, 5, 6, 8, 9]
            .into_iter()
            .all(|n| bytes[n].is_ascii_digit());
    if valid
        && let (Ok(year), Ok(month), Ok(day)) =
            (value[..4].parse(), value[5..7].parse(), value[8..].parse())
        && let Some(date) = NaiveDate::from_ymd_opt(year, month, day)
    {
        return Ok(date);
    }
    Err(DailyDocumentError::new(format!(
        "Invalid local date: {value}"
    )))
}
pub(super) fn unsafe_segment(value: &str) -> bool {
    value.is_empty()
        || value == "."
        || value == ".."
        || value
            .chars()
            .any(|c| matches!(c, '/' | '\\' | ':' | '\0'..='\u{001f}'))
}
pub fn format_daily_document_filename(
    local_date: &str,
    format: &str,
    context: &DailyFormatContext,
) -> Result<String, DailyDocumentError> {
    context.validate()?;
    let format = crate::text::trim(format);
    if format.is_empty() {
        return Err(DailyDocumentError::new(
            "Daily-note date format must not be empty.",
        ));
    }
    let date = date(local_date)?;
    let mut output = String::new();
    let mut current = String::new();
    let mut previous = None;
    let mut quoted = false;
    let append = |output: &mut String, current: &str, literal: bool| {
        if literal {
            output.push_str(current);
        } else {
            output.push_str(&tokens::render(date, current, context));
        }
    };
    for c in format.chars() {
        if c == '\'' {
            if !current.is_empty() || quoted {
                append(
                    &mut output,
                    if current.is_empty() { "'" } else { &current },
                    quoted,
                );
            }
            previous = None;
            current.clear();
            quoted = !quoted;
        } else if quoted || previous == Some(c) {
            current.push(c);
        } else {
            if !current.is_empty() {
                append(&mut output, &current, false);
            }
            current.clear();
            current.push(c);
            previous = Some(c);
        }
    }
    append(&mut output, &current, quoted);
    if unsafe_segment(&output) {
        return Err(DailyDocumentError::new(format!(
            "Daily-note date format must produce one safe filename segment; received {}.",
            serde_json::to_string(&output).expect("UTF-8 string serializes")
        )));
    }
    Ok(format!("{output}.md"))
}
