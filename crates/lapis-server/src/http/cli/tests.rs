use super::*;
use serde_json::json;
fn manifest() -> Value {
    let version = "2.0.3";
    let prefix = "https://github.com/lapismd/lapis-server/releases/download/v2.0.3/";
    let archives: Vec<_> = [("aarch64-apple-darwin", "macos-arm64"), ("aarch64-unknown-linux-gnu", "linux-arm64"), ("x86_64-unknown-linux-gnu", "linux-x64")].into_iter().map(|(target, label)| {
        let file = format!("lapis-v{version}-{label}.tar.gz");
        json!({"version": version,"sourceCommit":"a".repeat(40),"target":target,"file":file,"url":format!("{prefix}{file}"),"sha256":"b".repeat(64),"size":123})
    }).collect();
    json!({"schemaVersion":1,"version":version,"sourceCommit":"a".repeat(40),"archives":archives})
}
#[test]
fn download_manifest_requires_complete_exact_release_coordinates() {
    let prefix = "https://github.com/lapismd/lapis-server/releases/download/v2.0.3/";
    assert!(validate(&manifest(), "2.0.3", prefix).is_ok());
    for field in ["url", "file", "sha256", "sourceCommit", "version", "target", "size"] {
        let mut altered = manifest();
        altered["archives"][0][field] = json!("untrusted");
        assert!(validate(&altered, "2.0.3", prefix).is_err(), "{field}");
    }
    let mut duplicate = manifest();
    duplicate["archives"][1] = duplicate["archives"][0].clone();
    assert!(validate(&duplicate, "2.0.3", prefix).is_err());
    assert!(validate(&manifest(), "2.0.4", prefix).is_err());
}
