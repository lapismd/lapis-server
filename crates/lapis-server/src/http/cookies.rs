use crate::control::SESSION_IDLE_MS;
use axum::http::HeaderMap;

pub(super) fn name(insecure: bool) -> &'static str {
    if insecure {
        "lapis_session"
    } else {
        "__Host-lapis_session"
    }
}
pub(super) fn read(headers: &HeaderMap, insecure: bool) -> Option<&str> {
    let name = name(insecure);
    headers
        .get("cookie")?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|part| {
            let (key, value) = part.trim().split_once('=')?;
            (key == name).then_some(value.trim())
        })
}
pub(super) fn issue(token: &str, insecure: bool) -> String {
    value(token, insecure, SESSION_IDLE_MS / 1000)
}
pub(super) fn clear(insecure: bool) -> String {
    value("", insecure, 0)
}
fn value(token: &str, insecure: bool, age: i64) -> String {
    format!(
        "{}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={age}{}",
        name(insecure),
        if insecure { "" } else { "; Secure" }
    )
}
