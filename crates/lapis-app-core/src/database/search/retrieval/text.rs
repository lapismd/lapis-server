use super::query::{Kind, Literal};
use lapis_app_protocol::database::search::retrieval::SearchRange;

pub(super) fn whitespace(c: char) -> bool {
    matches!(c,'\u{0009}'..='\u{000d}'|'\u{0020}'|'\u{00a0}'|'\u{1680}'|'\u{2000}'..='\u{200a}'|'\u{2028}'|'\u{2029}'|'\u{202f}'|'\u{205f}'|'\u{3000}'|'\u{feff}')
}
pub(super) fn len(text: &str) -> usize {
    text.encode_utf16().count()
}
pub(super) fn bounds(input: &str, start: usize, end: usize) -> (usize, usize) {
    let units = input.encode_utf16().collect::<Vec<_>>();
    let mut start = start.min(units.len());
    let mut end = end.max(start).min(units.len());
    let splits = |n: usize| {
        n > 0
            && n < units.len()
            && (0xd800..=0xdbff).contains(&units[n - 1])
            && (0xdc00..=0xdfff).contains(&units[n])
    };
    if splits(start) {
        start -= 1
    }
    if splits(end) {
        end += 1
    }
    (start, end)
}
pub(super) fn slice(input: &str, start: usize, end: usize) -> String {
    let (start, end) = bounds(input, start, end);
    let units = input.encode_utf16().collect::<Vec<_>>();
    String::from_utf16(&units[start..end]).expect("scalar-safe source bounds")
}
pub(super) fn literal(text: &str, literal: &Literal, sensitive: bool) -> Vec<SearchRange> {
    match literal.kind {
        Kind::Null => vec![],
        Kind::Regex => {
            let Some(regex) = literal.regex[usize::from(sensitive)].as_ref() else {
                return vec![];
            };
            let units: Vec<_> = text.encode_utf16().collect();
            regex
                .find_from_utf16(&units, 0)
                .filter(|m| m.range.end > m.range.start)
                .take(64)
                .map(|m| SearchRange {
                    start: m.range.start,
                    end: m.range.end,
                })
                .collect()
        }
        _ => ranges(text, &literal.text, sensitive),
    }
}
pub(super) fn ranges(text: &str, term: &str, sensitive: bool) -> Vec<SearchRange> {
    let size = len(term);
    if size == 0 {
        return vec![];
    }
    let haystack: Vec<_> = if sensitive {
        text.to_owned()
    } else {
        text.to_lowercase()
    }
    .encode_utf16()
    .collect();
    let needle: Vec<_> = if sensitive {
        term.to_owned()
    } else {
        term.to_lowercase()
    }
    .encode_utf16()
    .collect();
    let mut result = vec![];
    let mut cursor = 0;
    while cursor < haystack.len() {
        let Some(index) = haystack[cursor..]
            .windows(needle.len())
            .position(|v| v == needle)
        else {
            break;
        };
        let start = cursor + index;
        result.push(SearchRange {
            start,
            end: start + size,
        });
        cursor = start + size;
        if result.len() == 64 {
            break;
        }
    }
    result
}
pub(super) fn merge(mut ranges: Vec<SearchRange>) -> Vec<SearchRange> {
    ranges.sort_by_key(|r| (r.start, r.end));
    let mut out: Vec<SearchRange> = vec![];
    for range in ranges {
        if let Some(previous) = out.last_mut().filter(|p| range.start <= p.end) {
            previous.end = previous.end.max(range.end)
        } else {
            out.push(range)
        }
    }
    out
}
