use super::{DailyDocumentError, DailyFormatContext, format};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DailyDocumentSettings {
    pub folder: String,
    pub date_format: String,
}
impl Default for DailyDocumentSettings {
    fn default() -> Self {
        Self {
            folder: "daily".into(),
            date_format: "yyyy-MM-dd".into(),
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DailyDocumentCandidate {
    pub path: String,
    pub frontmatter: Value,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DailyDocumentPlan {
    Existing { path: String },
    Create { path: String, text: String },
}
pub fn plan_daily_document(
    date: &str,
    create: bool,
    candidates: &[DailyDocumentCandidate],
    settings: &DailyDocumentSettings,
    context: &DailyFormatContext,
) -> Result<Option<DailyDocumentPlan>, DailyDocumentError> {
    let existing = locate(date, candidates)?;
    if existing.is_some() || !create {
        return Ok(existing);
    }
    create_plan(date, candidates, settings, context).map(Some)
}
pub(super) fn locate(
    date: &str,
    candidates: &[DailyDocumentCandidate],
) -> Result<Option<DailyDocumentPlan>, DailyDocumentError> {
    format::date(date)?;
    let canonical = candidates
        .iter()
        .filter(|candidate| {
            candidate.frontmatter["type"] == "daily-note" && candidate.frontmatter["date"] == date
        })
        .collect::<Vec<_>>();
    if canonical.len() > 1 {
        return Err(DailyDocumentError::new(format!(
            "Multiple daily notes declare date {date}: {}",
            canonical
                .iter()
                .map(|candidate| candidate.path.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    if let Some(candidate) = canonical.first() {
        return Ok(Some(DailyDocumentPlan::Existing {
            path: candidate.path.clone(),
        }));
    }
    Ok(None)
}
fn create_plan(
    date: &str,
    candidates: &[DailyDocumentCandidate],
    settings: &DailyDocumentSettings,
    context: &DailyFormatContext,
) -> Result<DailyDocumentPlan, DailyDocumentError> {
    let folder = crate::text::trim(&settings.folder).replace('\\', "/");
    let folder = folder.trim_matches('/');
    if folder.is_empty()
        || folder
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || folder.chars().any(|c| matches!(c, ':' | '\0'..='\u{001f}'))
    {
        return Err(DailyDocumentError::new(format!(
            "Invalid daily-note folder: {}.",
            serde_json::to_string(&settings.folder).expect("UTF-8 string serializes")
        )));
    }
    let filename = format::format_daily_document_filename(date, &settings.date_format, context)?;
    let path = format!("{folder}/{filename}");
    if candidates.iter().any(|candidate| candidate.path == path) {
        return Err(DailyDocumentError::new(format!(
            "Cannot create daily note for {date}: {path} already exists without matching daily-note front matter."
        )));
    }
    Ok(DailyDocumentPlan::Create {
        path,
        text: format!("---\ntype: daily-note\ndate: {date}\n---\n\n# {date}\n"),
    })
}
