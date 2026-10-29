use super::{evaluation, query::Query, text};
use lapis_app_protocol::database::search::{records::SearchDocumentRecord, retrieval::*};
use std::collections::BTreeMap;

fn snippet(
    field: SearchField,
    text: String,
    ranges: Vec<SearchRange>,
    offset: usize,
) -> SearchSnippet {
    SearchSnippet {
        field,
        text,
        ranges,
        offset: offset as f64,
        chunk_id: None,
        chunk_label: None,
    }
}
fn clamp(ranges: &[SearchRange], start: usize, end: usize) -> Vec<SearchRange> {
    ranges
        .iter()
        .filter_map(|r| {
            let begin = r.start.saturating_sub(start);
            let finish = r.end.min(end).saturating_sub(start);
            (finish > begin).then_some(SearchRange {
                start: begin,
                end: finish,
            })
        })
        .collect()
}
fn field(
    field: SearchField,
    input: &str,
    ranges: Vec<SearchRange>,
    limit: usize,
) -> Vec<SearchSnippet> {
    if ranges.is_empty() || input.is_empty() {
        return vec![];
    }
    if text::len(input) <= limit || matches!(field, SearchField::Name | SearchField::Path) {
        return vec![snippet(field, input.into(), ranges, 0)];
    }
    let focus = &ranges[0];
    let padding = (limit / 3).max(16);
    let start = focus.start.saturating_sub(padding);
    let end = text::len(input).min((start + limit).max(focus.end + padding));
    let (start, end) = text::bounds(input, start, end);
    vec![snippet(
        field,
        text::slice(input, start, end),
        clamp(&ranges, start, end),
        start,
    )]
}
fn content(input: &str, ranges: Vec<SearchRange>, limit: usize) -> Vec<SearchSnippet> {
    if ranges.is_empty() || input.is_empty() {
        return vec![];
    }
    let units: Vec<_> = input.encode_utf16().collect();
    let mut lines: BTreeMap<(usize, usize), Vec<SearchRange>> = BTreeMap::new();
    for range in ranges {
        let before = (range.start.saturating_sub(1) + 1).min(units.len());
        let start = units[..before]
            .iter()
            .rposition(|c| *c == 10)
            .map_or(0, |n| n + 1);
        let after = range.end.min(units.len());
        let end = units[after..]
            .iter()
            .position(|c| *c == 10)
            .map_or(units.len(), |n| n + after);
        lines
            .entry((start, end))
            .or_default()
            .extend(clamp(&[range], start, end));
    }
    lines
        .into_iter()
        .take(8)
        .map(|((offset, end), ranges)| {
            let input = text::slice(input, offset, end);
            let ranges = text::merge(ranges);
            if text::len(&input) <= limit {
                return snippet(SearchField::Content, input, ranges, offset);
            }
            let focus = &ranges[0];
            let padding = limit / 3;
            let start = focus.start.saturating_sub(padding);
            let end = text::len(&input).min((start + limit).max(focus.end + padding));
            let (start, end) = text::bounds(&input, start, end);
            snippet(
                SearchField::Content,
                text::slice(&input, start, end),
                clamp(&ranges, start, end),
                offset + start,
            )
        })
        .collect()
}
fn matching(
    doc: &SearchDocumentRecord,
    query: &Query,
    options: &SearchOptions,
) -> Vec<SearchSnippet> {
    let limit = options.snippet_length.unwrap_or(160);
    let sensitive = options.case_sensitive.unwrap_or(false);
    let mut result = vec![];
    for (f, input) in [
        (SearchField::Name, doc.name.clone()),
        (SearchField::Path, doc.path.clone()),
        (
            SearchField::Tags,
            doc.tags
                .iter()
                .map(|t| format!("#{t}"))
                .collect::<Vec<_>>()
                .join(" "),
        ),
    ] {
        result.extend(field(
            f,
            &input,
            evaluation::ranges(query, &input, f, sensitive),
            limit,
        ));
    }
    if let Some(chunks) = doc.chunks.as_ref().filter(|c| !c.is_empty()) {
        let mut snippets = vec![];
        for chunk in chunks {
            for mut snippet in content(
                &chunk.text,
                evaluation::ranges(query, &chunk.text, SearchField::Content, sensitive),
                limit,
            ) {
                snippet.offset += chunk.start_offset;
                snippet.chunk_id = Some(chunk.id.clone());
                snippet.chunk_label = chunk.heading.clone();
                snippets.push(snippet);
            }
        }
        snippets.sort_by(|a, b| a.offset.total_cmp(&b.offset));
        result.extend(snippets.into_iter().take(8));
    } else {
        result.extend(content(
            &doc.content,
            evaluation::ranges(query, &doc.content, SearchField::Content, sensitive),
            limit,
        ))
    }
    let meta = doc.metadata_text.as_deref().unwrap_or_default();
    result.extend(field(
        SearchField::Metadata,
        meta,
        evaluation::ranges(query, meta, SearchField::Metadata, sensitive),
        limit,
    ));
    result
}
pub(super) fn build(
    doc: &SearchDocumentRecord,
    query: &Query,
    options: &SearchOptions,
) -> Vec<SearchSnippet> {
    semantic(doc, query, options, 1., &[])
}
pub(super) fn semantic(
    doc: &SearchDocumentRecord,
    query: &Query,
    options: &SearchOptions,
    lexical: f64,
    preferred: &[String],
) -> Vec<SearchSnippet> {
    let mut result = matching(doc, query, options);
    if result.is_empty() && lexical > 0. {
        result.push(snippet(SearchField::Name, doc.name.clone(), vec![], 0));
    }
    if result.is_empty() {
        for id in preferred.iter().take(3) {
            if let Some(chunk) = doc.chunks.iter().flatten().find(|c| &c.id == id) {
                let mut preview = snippet(
                    SearchField::Content,
                    text::slice(&chunk.text, 0, options.snippet_length.unwrap_or(160)),
                    vec![],
                    0,
                );
                preview.offset = chunk.start_offset;
                preview.chunk_id = Some(chunk.id.clone());
                preview.chunk_label = chunk.heading.clone();
                result.push(preview);
            }
        }
    }
    result
}
