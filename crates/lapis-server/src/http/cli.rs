//! Fixed public release metadata; never forwards owner credentials upstream.
use super::{State, errors::Failure};
use axum::{Json, extract::State as AxumState};
use futures_util::StreamExt;
use lapis_app_protocol::ErrorCode;
use serde_json::Value;

pub(super) async fn downloads(AxumState(state): AxumState<State>) -> Result<Json<Value>, Failure> {
    let package: Value = serde_json::from_str(include_str!("../../../../package.json"))
        .map_err(|_| unavailable())?;
    let version = package["version"].as_str().ok_or_else(unavailable)?;
    let prefix = format!("https://github.com/lapismd/lapis-server/releases/download/v{version}/");
    let response = state
        .downloads
        .get(format!("{prefix}cli-manifest.json"))
        .header("accept", "application/octet-stream")
        .send()
        .await
        .map_err(|_| unavailable())?;
    if !response.status().is_success() || response.content_length().is_some_and(|n| n > 16384) {
        return Err(unavailable());
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| unavailable())?;
        if bytes.len() + chunk.len() > 16384 {
            return Err(unavailable());
        }
        bytes.extend_from_slice(&chunk);
    }
    let manifest: Value = serde_json::from_slice(&bytes).map_err(|_| unavailable())?;
    validate(&manifest, version, &prefix).map_err(|_| unavailable())?;
    Ok(Json(manifest["archives"].clone()))
}
fn unavailable() -> Failure {
    Failure::new(
        ErrorCode::Unsupported,
        "CLI downloads for this server release are not available yet. Use the bundled desktop CLI or retry after release publication.",
    )
}
fn validate(manifest: &Value, version: &str, prefix: &str) -> Result<(), ()> {
    let archives = manifest["archives"].as_array().ok_or(())?;
    if manifest["schemaVersion"] != 1 || manifest["version"] != version || archives.len() != 3 {
        return Err(());
    }
    let source = manifest["sourceCommit"].as_str().ok_or(())?;
    if source.len() != 40 || !source.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(());
    }
    for (target, label) in [
        ("aarch64-apple-darwin", "macos-arm64"),
        ("aarch64-unknown-linux-gnu", "linux-arm64"),
        ("x86_64-unknown-linux-gnu", "linux-x64"),
    ] {
        let matches: Vec<_> = archives.iter().filter(|a| a["target"] == target).collect();
        if matches.len() != 1 {
            return Err(());
        }
        let artifact = matches[0];
        let file = format!("lapis-v{version}-{label}.tar.gz");
        let checksum = artifact["sha256"].as_str().ok_or(())?;
        if artifact["version"] != version
            || artifact["sourceCommit"] != source
            || artifact["file"] != file
            || artifact["url"] != format!("{prefix}{file}")
            || checksum.len() != 64
            || !checksum.bytes().all(|b| b.is_ascii_hexdigit())
            || artifact["size"].as_u64().unwrap_or(0) == 0
        {
            return Err(());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
