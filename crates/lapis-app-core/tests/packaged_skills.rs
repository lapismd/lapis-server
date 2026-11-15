use lapis_app_core::{
    skills::{PackagedSkill, manage},
    vault::{VaultRoot, WriteOptions},
};
use std::collections::BTreeMap;
fn bundle(body: &str) -> PackagedSkill {
    PackagedSkill {
        name: "lapis-example".into(),
        source: "example".into(),
        version: body.into(),
        files: BTreeMap::from([
            ("SKILL.md".into(), body.into()),
            ("references/example.md".into(), body.into()),
        ]),
    }
}
#[test]
fn fresh_install_clean_update_and_cold_modified_skip() {
    let temp = tempfile::tempdir().unwrap();
    let root = VaultRoot::open(temp.path()).unwrap();
    assert_eq!(
        manage(&root, &[bundle("one")], "install", None).unwrap()[0].status,
        "installed"
    );
    assert_eq!(
        manage(&root, &[bundle("one")], "install", None).unwrap()[0].status,
        "unchanged"
    );
    assert_eq!(
        manage(&root, &[bundle("two")], "update", None).unwrap()[0].status,
        "updated"
    );
    root.write(
        ".agents/skills/lapis-example/references/example.md",
        b"local",
        &WriteOptions::default(),
    )
    .unwrap();
    drop(root);
    let root = VaultRoot::open(temp.path()).unwrap();
    let result = manage(&root, &[bundle("three")], "update", None).unwrap();
    assert_eq!(result[0].reason.as_deref(), Some("locally-modified"));
    assert_eq!(
        root.read_text(".agents/skills/lapis-example/SKILL.md")
            .unwrap(),
        "two"
    );
}
#[test]
fn extra_deleted_and_unmanaged_files_preserve_the_whole_skill() {
    for mutation in ["added", "deleted", "unmanaged"] {
        let temp = tempfile::tempdir().unwrap();
        let root = VaultRoot::open(temp.path()).unwrap();
        manage(&root, &[bundle("one")], "install", None).unwrap();
        match mutation {
            "added" => root
                .write(
                    ".agents/skills/lapis-example/mine.md",
                    b"mine",
                    &WriteOptions::default(),
                )
                .unwrap(),
            "deleted" => root
                .remove_file(".agents/skills/lapis-example/references/example.md")
                .unwrap(),
            _ => root.remove_file(".agents/.lapis-skills.json").unwrap(),
        }
        assert_eq!(
            manage(&root, &[bundle("two")], "update", None).unwrap()[0].status,
            "skipped"
        );
        assert_eq!(
            root.read_text(".agents/skills/lapis-example/SKILL.md")
                .unwrap(),
            "one"
        );
    }
}
#[test]
fn pending_corrupt_duplicate_and_traversal_fail_closed() {
    let temp = tempfile::tempdir().unwrap();
    let root = VaultRoot::open(temp.path()).unwrap();
    assert!(manage(&root, &[bundle("one"), bundle("two")], "install", None).is_err());
    let mut bad = bundle("one");
    bad.files.insert("../escape".into(), "bad".into());
    assert!(manage(&root, &[bad], "install", None).is_err());
    manage(&root, &[bundle("one")], "install", None).unwrap();
    let receipt = root
        .read_text(".agents/.lapis-skills.json")
        .unwrap()
        .replace("\"pending\": false", "\"pending\": true");
    root.write(
        ".agents/.lapis-skills.json",
        receipt.as_bytes(),
        &WriteOptions::default(),
    )
    .unwrap();
    assert_eq!(
        manage(&root, &[bundle("two")], "update", None).unwrap()[0]
            .reason
            .as_deref(),
        Some("interrupted")
    );
    root.write(
        ".agents/.lapis-skills.json",
        b"broken",
        &WriteOptions::default(),
    )
    .unwrap();
    assert!(manage(&root, &[bundle("two")], "update", None).is_err());
}

#[test]
fn binary_local_changes_skip_the_entire_skill() {
    for file in ["mine.png", "references/example.md"] {
        let temp = tempfile::tempdir().unwrap();
        let root = VaultRoot::open(temp.path()).unwrap();
        manage(&root, &[bundle("one")], "install", None).unwrap();
        let path = format!(".agents/skills/lapis-example/{file}");
        let bytes = [0xff, 0x00, 0xfe];
        root.write(&path, &bytes, &WriteOptions::default()).unwrap();
        let result = manage(&root, &[bundle("two")], "update", None).unwrap();
        assert_eq!(result[0].status, "skipped");
        assert_eq!(result[0].reason.as_deref(), Some("locally-modified"));
        assert_eq!(root.read(&path).unwrap(), bytes);
        assert_eq!(
            root.read_text(".agents/skills/lapis-example/SKILL.md")
                .unwrap(),
            "one"
        );
    }
}
#[cfg(unix)]
#[test]
fn symlinks_including_untracked_children_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let root = VaultRoot::open(temp.path()).unwrap();
    manage(&root, &[bundle("one")], "install", None).unwrap();
    std::os::unix::fs::symlink(
        outside.path(),
        temp.path().join(".agents/skills/lapis-example/link"),
    )
    .unwrap();
    assert!(manage(&root, &[bundle("two")], "update", None).is_err());
    assert_eq!(
        root.read_text(".agents/skills/lapis-example/SKILL.md")
            .unwrap(),
        "one"
    );
}

#[test]
fn skill_permissions_never_upgrade_an_existing_read_grant() {
    use lapis_app_core::services::{
        Runtime,
        content::{ContentGrants, ContentVault},
        plugin_operations::OperationRegistry,
    };
    use lapis_app_core::skills::provider::SkillsProvider;
    use lapis_app_protocol::operations::*;
    use std::{sync::Arc, time::Duration};
    use uuid::Uuid;
    let temp = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let id = Uuid::new_v4();
    let owner = runtime.admit(id, temp.path()).unwrap();
    let mut registry = OperationRegistry::default();
    registry
        .register(Arc::new(SkillsProvider::default()))
        .unwrap();
    let grants = ContentGrants::with_operations(Arc::new(registry));
    grants.set_enabled(true).unwrap();
    let issue = |permissions: &[String]| {
        grants
            .issue_named_operations(
                "skills",
                vec![ContentVault {
                    name: "vault".into(),
                    client: owner.attach().unwrap(),
                }],
                Duration::from_secs(60),
                permissions,
            )
            .unwrap()
    };
    let read = issue(&[]);
    assert!(
        grants
            .operation_catalog(read.token.expose_secret(), id)
            .unwrap()
            .operations
            .is_empty()
    );
    let request = || OperationRequest {
        protocol_version: 1,
        request_id: Uuid::new_v4(),
        version: 1,
        input: serde_json::json!({}),
    };
    assert!(matches!(
        grants
            .invoke_operation(
                read.token.expose_secret(),
                id,
                "skills",
                "install",
                request()
            )
            .outcome,
        OperationOutcome::Error { .. }
    ));
    assert!(!temp.path().join(".agents").exists());
    let write = issue(&["skills:write".into(), "skills:read".into()]);
    let result = grants.invoke_operation(
        write.token.expose_secret(),
        id,
        "skills",
        "install",
        request(),
    );
    assert!(
        matches!(result.outcome, OperationOutcome::Ok { .. }),
        "{result:?}"
    );
    assert!(
        temp.path()
            .join(".agents/skills/lapis-cli/SKILL.md")
            .is_file()
    );
}

#[test]
fn disabled_bundled_skills_are_excluded_without_an_installed_plugin_registry() {
    let directory = tempfile::tempdir().unwrap();
    let root = VaultRoot::open(directory.path()).unwrap();
    root.mkdir(".lapis", true).unwrap();
    root.write(
        ".lapis/core-plugins.json",
        br#"["example"]"#,
        &WriteOptions::default(),
    )
    .unwrap();
    let sources = lapis_app_core::skills::sources::discover(&root, &[bundle("one")]).unwrap();
    assert!(sources.is_empty());
}

#[test]
fn revoked_installed_sources_are_not_read() {
    let directory = tempfile::tempdir().unwrap();
    let root = VaultRoot::open(directory.path()).unwrap();
    root.mkdir(".lapis", true).unwrap();
    root.write(
        ".lapis/community-plugins.json",
        br#"["example"]"#,
        &WriteOptions::default(),
    )
    .unwrap();
    root.write(".lapis/installed-plugins.json", br#"{"schemaVersion":1,"plugins":{"example":{"pluginId":"example","installedVersion":"1.0.0","files":[],"revoked":{"reason":"withdrawn"}}}}"#, &WriteOptions::default()).unwrap();
    assert!(
        lapis_app_core::skills::sources::discover(&root, &[])
            .unwrap()
            .is_empty()
    );
}
