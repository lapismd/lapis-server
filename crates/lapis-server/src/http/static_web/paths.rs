use axum::http::StatusCode;

pub(super) fn decode(raw: &str) -> Result<String, StatusCode> {
    if raw.len() > 4096 {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut bytes = Vec::with_capacity(raw.len());
    let mut source = raw.bytes();
    while let Some(byte) = source.next() {
        if byte == b'%' {
            let high = source.next().and_then(hex).ok_or(StatusCode::BAD_REQUEST)?;
            let low = source.next().and_then(hex).ok_or(StatusCode::BAD_REQUEST)?;
            bytes.push(high * 16 + low);
        } else {
            bytes.push(byte);
        }
    }
    let decoded = String::from_utf8(bytes).map_err(|_| StatusCode::BAD_REQUEST)?;
    if decoded
        .chars()
        .any(|c| c.is_control() || matches!(c, '\\' | ':' | '%'))
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    if decoded != "/" && decoded.ends_with("//") {
        return Err(StatusCode::NOT_FOUND);
    }
    let relative = decoded.strip_prefix('/').ok_or(StatusCode::BAD_REQUEST)?;
    let relative = relative.strip_suffix('/').unwrap_or(relative);
    if !relative.is_empty()
        && relative
            .split('/')
            .any(|part| part.is_empty() || part.starts_with('.'))
    {
        return Err(StatusCode::NOT_FOUND);
    }
    Ok(relative.into())
}
fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}
pub(super) fn reserved(path: &str) -> bool {
    matches!(
        path.split('/').next(),
        Some("api" | "__lapis" | "healthz" | "readyz")
    )
}
pub(super) fn navigation(path: &str) -> bool {
    !path.contains('.') && !matches!(path.split('/').next(), Some("assets"))
}
pub(super) fn cache(path: &str) -> &'static str {
    if path
        .split('.')
        .any(|part| part.len() >= 8 && part.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    }
}
