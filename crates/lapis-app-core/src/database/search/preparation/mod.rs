mod breakpoints;
pub(super) mod json;
use super::validation;
use crate::database::{MAX_BYTES, encoded, invalid};
use crate::services::Result;
use lapis_app_protocol::database::search::{records::*, source::SearchSourceMetadata};
use serde_json::Value;
use std::collections::HashSet;

pub(super) fn normalize(mut record: SearchDocumentRecord) -> Result<SearchDocumentRecord> {
    validation::record(&record)?;
    encoded(&record)?;
    let Some(source) = &record.source_metadata else {
        return Ok(record);
    };
    let text: Vec<u16> = record.content.encode_utf16().collect();
    settings(source, &text)?;
    record.tags = unique(source.raw_tags.iter().cloned())
        .into_iter()
        .filter(|tag| !trim(tag).is_empty())
        .map(|tag| tag.strip_prefix('#').unwrap_or(&tag).to_owned())
        .collect();
    let mut parts = vec![];
    let mut hierarchy = vec![];
    for tag in &record.tags {
        let components: Vec<_> = tag.split('/').filter(|p| !trim(p).is_empty()).collect();
        parts.extend(components.iter().map(|s| (*s).to_owned()));
        for end in 1..=components.len() {
            hierarchy.push(components[..end].join("/"));
        }
    }
    record.tag_parts = unique(parts);
    record.tag_hierarchy = unique(hierarchy);
    record.metadata_text = Some(match &source.frontmatter {
        None => record.metadata_text.unwrap_or_default(),
        Some(Value::Null) => "{}".into(),
        Some(value) => json::stringify(value),
    });
    record.chunks = Some(chunks(&record.path, &text, source)?);
    encoded(&record)?;
    Ok(record)
}
fn unique(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = HashSet::new();
    values
        .into_iter()
        .filter(|v| seen.insert(v.clone()))
        .collect()
}
pub(super) fn validate_chunking(
    chunking: &lapis_app_protocol::database::search::source::SearchChunkingSettings,
) -> Result<()> {
    let integer = |n: f64| n >= 0. && n.fract() == 0. && n <= usize::MAX as f64;
    if chunking.target_chars < 1.
        || !integer(chunking.target_chars)
        || !integer(chunking.breakpoint_window_chars)
    {
        return Err(invalid("Invalid Search chunk settings"));
    }
    if !chunking.breakpoint_decay.is_finite() {
        return Err(invalid("Invalid Search numeric data"));
    }
    Ok(())
}
fn settings(source: &SearchSourceMetadata, text: &[u16]) -> Result<()> {
    validate_chunking(&source.chunking)?;
    let integer = |n: f64| n >= 0. && n.fract() == 0. && n <= usize::MAX as f64;
    let offsets = source
        .headings
        .iter()
        .flat_map(|h| [h.position.start.offset, h.position.end.offset])
        .chain(
            source
                .sections
                .iter()
                .flat_map(|s| [s.position.start.offset, s.position.end.offset]),
        )
        .chain(source.frontmatter_end_offset);
    for offset in offsets {
        if !integer(offset) || split_pair(text, offset as usize) {
            return Err(invalid("Invalid Search source offset"));
        }
    }
    Ok(())
}
fn chunks(
    path: &str,
    text: &[u16],
    source: &SearchSourceMetadata,
) -> Result<Vec<SearchDocumentChunk>> {
    let mut start = source.frontmatter_end_offset.unwrap_or(0.) as usize;
    if start >= text.len() {
        return Ok(vec![]);
    }
    let points = breakpoints::points(text, source, start);
    let fences = breakpoints::fences(text, source, start);
    let mut headings: Vec<_> = source.headings.iter().collect();
    headings.sort_by(|a, b| a.position.start.offset.total_cmp(&b.position.start.offset));
    let mut result = vec![];
    let mut bytes = 0usize;
    let target_size = source.chunking.target_chars as usize;
    while start < text.len() {
        let target = start.saturating_add(target_size).min(text.len());
        let mut end = if text.len() - start > target_size {
            breakpoints::cutoff(
                &points,
                start,
                target,
                source.chunking.breakpoint_window_chars as usize,
                source.chunking.breakpoint_decay,
                &fences,
            )
        } else {
            text.len()
        };
        if end <= start {
            end = target;
        }
        if split_pair(text, end) {
            end += 1;
        }
        let mut trimmed_start = start;
        let mut trimmed_end = end;
        while trimmed_start < trimmed_end && whitespace(text[trimmed_start]) {
            trimmed_start += 1;
        }
        while trimmed_end > trimmed_start && whitespace(text[trimmed_end - 1]) {
            trimmed_end -= 1;
        }
        if trimmed_end > trimmed_start {
            let section = source.sections.iter().find(|s| {
                s.position.start.offset <= trimmed_start as f64
                    && s.position.end.offset >= trimmed_start as f64
            });
            let kind = match section.map(|s| s.r#type.as_str()) {
                Some("heading") => SearchChunkKind::Heading,
                Some("paragraph") => SearchChunkKind::Paragraph,
                Some("list") => SearchChunkKind::List,
                Some("blockquote" | "callout") => SearchChunkKind::Blockquote,
                Some("code") => SearchChunkKind::Code,
                _ => SearchChunkKind::Fallback,
            };
            let chunk = SearchDocumentChunk {
                id: format!("{path}#chunk-{trimmed_start}-{trimmed_end}"),
                text: String::from_utf16(&text[trimmed_start..trimmed_end])
                    .map_err(|_| invalid("Invalid Search source text"))?,
                start_offset: trimmed_start as f64,
                end_offset: trimmed_end as f64,
                heading: headings
                    .iter()
                    .take_while(|h| h.position.start.offset <= trimmed_start as f64)
                    .last()
                    .map(|h| h.heading.clone()),
                kind: Some(kind),
                embedding: None,
            };
            bytes = bytes.saturating_add(encoded(&chunk)?.len());
            if bytes >= MAX_BYTES {
                return Err(invalid("Prepared Search chunks exceed bounds"));
            }
            result.push(chunk);
        }
        start = end;
    }
    Ok(result)
}
fn split_pair(text: &[u16], offset: usize) -> bool {
    offset > 0
        && offset < text.len()
        && (0xd800..=0xdbff).contains(&text[offset - 1])
        && (0xdc00..=0xdfff).contains(&text[offset])
}
fn whitespace(unit: u16) -> bool {
    matches!(unit, 0x0009..=0x000d | 0x0020 | 0x00a0 | 0x1680 | 0x2000..=0x200a | 0x2028 | 0x2029 | 0x202f | 0x205f | 0x3000 | 0xfeff)
}
fn trim(value: &str) -> &str {
    value.trim_matches(|c: char| c.len_utf16() == 1 && whitespace(c as u16))
}
