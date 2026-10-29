use axum::http::{HeaderMap, StatusCode};

pub(in crate::http) struct Selection {
    pub start: u64,
    pub length: u64,
    pub status: StatusCode,
}
pub(in crate::http) fn select(
    headers: &HeaderMap,
    length: u64,
    etag: &str,
) -> Result<Selection, ()> {
    let range = headers
        .get("range")
        .filter(|_| headers.get("if-range").is_none_or(|value| value == etag));
    let Some(range) = range else {
        return Ok(Selection {
            start: 0,
            length,
            status: StatusCode::OK,
        });
    };
    let value = range
        .to_str()
        .map_err(|_| ())?
        .strip_prefix("bytes=")
        .ok_or(())?;
    let (start, end) = value.split_once('-').ok_or(())?;
    if length == 0 {
        return Err(());
    }
    let (start, end) = if start.is_empty() {
        let suffix = number(end)?;
        if suffix == 0 {
            return Err(());
        }
        (length.saturating_sub(suffix), length - 1)
    } else {
        let start = number(start)?;
        let end = if end.is_empty() {
            length - 1
        } else {
            number(end)?.min(length - 1)
        };
        if start >= length || end < start {
            return Err(());
        }
        (start, end)
    };
    Ok(Selection {
        start,
        length: end - start + 1,
        status: StatusCode::PARTIAL_CONTENT,
    })
}
fn number(value: &str) -> Result<u64, ()> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(());
    }
    value.parse().map_err(|_| ())
}
