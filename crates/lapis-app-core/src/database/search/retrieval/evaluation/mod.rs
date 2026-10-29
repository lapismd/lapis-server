mod document;
mod text;
use super::text as search_text;
pub(super) use document::score;
use lapis_app_protocol::database::search::{
    records::SearchDocumentRecord,
    retrieval::{SearchField, SearchRange},
};
pub(super) use text::ranges;

#[derive(Default)]
pub(super) struct Eval {
    pub matched: bool,
    pub score: f64,
    pub ranges: Vec<SearchRange>,
}
impl Eval {
    pub fn yes(score: f64, ranges: Vec<SearchRange>) -> Self {
        Self {
            matched: true,
            score,
            ranges,
        }
    }
}
pub(super) fn weight(field: SearchField) -> f64 {
    match field {
        SearchField::Name => 35.,
        SearchField::Path => 30.,
        SearchField::Tags => 25.,
        SearchField::Metadata => 18.,
        SearchField::Content => 12.,
    }
}
pub(super) fn mapped(name: &str) -> Option<SearchField> {
    match name {
        "file" => Some(SearchField::Name),
        "path" => Some(SearchField::Path),
        "tag" => Some(SearchField::Tags),
        "content" => Some(SearchField::Content),
        _ => None,
    }
}
pub(super) fn field(record: &SearchDocumentRecord, field: SearchField) -> String {
    match field {
        SearchField::Name => record.name.clone(),
        SearchField::Path => record.path.clone(),
        SearchField::Content => record.content.clone(),
        SearchField::Metadata => record.metadata_text.clone().unwrap_or_default(),
        SearchField::Tags => record
            .tags
            .iter()
            .cloned()
            .chain(record.tags.iter().map(|t| format!("#{t}")))
            .chain(record.tag_parts.clone())
            .chain(record.tag_hierarchy.clone())
            .collect::<Vec<_>>()
            .join(" "),
    }
}
pub(super) fn combine(items: Vec<Eval>, and: bool) -> Eval {
    if and && items.iter().any(|e| !e.matched) {
        return Eval::default();
    }
    let matches = items.into_iter().filter(|e| e.matched).collect::<Vec<_>>();
    if matches.is_empty() && !and {
        return Eval::default();
    }
    Eval::yes(
        matches.iter().map(|e| e.score).sum(),
        search_text::merge(matches.into_iter().flat_map(|e| e.ranges).collect()),
    )
}
