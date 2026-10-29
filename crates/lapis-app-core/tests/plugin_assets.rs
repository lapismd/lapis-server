use lapis_app_core::{plugins::assets, vault::VaultRoot};
use serde_json::json;
use sha2::{Digest, Sha256};

fn seed() -> (tempfile::TempDir, VaultRoot, String) {
    let folder = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(folder.path().join(".lapis/plugins/portable")).unwrap();
    let hash = "4bf5122f344554c53bde2ebb8cd2b7e3d1600ad631c385a5d7cce23c7785459a";
    let files: Vec<_> = ["z.js", "a.js", "A.js", "Z.js", "\u{e000}.js", "😀.js"]
        .into_iter()
        .map(|path| json!({"path":path,"size":1,"sha256":hash}))
        .collect();
    std::fs::write(folder.path().join(".lapis/installed-plugins.json"), json!({"schemaVersion":1,"plugins":{"portable":{"pluginId":"portable","installedVersion":"1.0.0","files":files}}}).to_string()).unwrap();
    std::fs::write(folder.path().join(".lapis/plugins/portable/A.js"), [1]).unwrap();
    let root = VaultRoot::open(folder.path()).unwrap();
    (
        folder,
        root,
        "19f31e9afb4637cb065a41d52283152f5cd4e0e8cf606a83dee7e9383766e22e".into(),
    )
}
#[test]
fn extracted_reader_retains_the_independent_public_utf16_fingerprint_vector() {
    let (_folder, root, fingerprint) = seed();
    let asset = assets::read(&root, "portable", "1.0.0", &fingerprint, "A.js").unwrap();
    assert_eq!(asset.bytes, [1]);
    assert_eq!(asset.content_type, "text/javascript; charset=utf-8");
    assert_eq!(asset.sha256, format!("{:x}", Sha256::digest([1])));
}
#[test]
fn extracted_reader_refuses_stale_bytes_metadata_and_unsafe_paths() {
    let (folder, root, fingerprint) = seed();
    for (plugin, version, path) in [
        ("portable", "2.0.0", "A.js"),
        ("missing", "1.0.0", "A.js"),
        ("portable", "1.0.0", "../A.js"),
        ("portable", "1.0.0", "A.html"),
    ] {
        assert!(assets::read(&root, plugin, version, &fingerprint, path).is_err());
    }
    std::fs::write(folder.path().join(".lapis/plugins/portable/A.js"), [2]).unwrap();
    assert!(assets::read(&root, "portable", "1.0.0", &fingerprint, "A.js").is_err());
    std::fs::write(
        folder.path().join(".lapis/installed-plugins.json"),
        vec![b' '; 4 * 1024 * 1024 + 1],
    )
    .unwrap();
    assert!(assets::read(&root, "portable", "1.0.0", &fingerprint, "A.js").is_err());
}
#[test]
fn extracted_reader_refuses_linked_and_replaced_content() {
    let (folder, root, fingerprint) = seed();
    let file = folder.path().join(".lapis/plugins/portable/A.js");
    std::fs::remove_file(&file).unwrap();
    std::fs::write(folder.path().join("private.js"), [1]).unwrap();
    std::os::unix::fs::symlink("../../../private.js", file).unwrap();
    assert!(assets::read(&root, "portable", "1.0.0", &fingerprint, "A.js").is_err());
    let renamed = folder.path().with_extension("replaced");
    std::fs::rename(folder.path(), &renamed).unwrap();
    std::fs::create_dir(folder.path()).unwrap();
    assert!(assets::read(&root, "portable", "1.0.0", &fingerprint, "A.js").is_err());
    std::fs::remove_dir_all(renamed).unwrap();
}
