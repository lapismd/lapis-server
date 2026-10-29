use lapis_app_core::{
    context::skill_catalog::*,
    services::{Runtime, VaultService},
};
use lapis_app_protocol::{ErrorCode, VaultPhase};
use std::{
    cell::Cell,
    fs,
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

const PATH: &str = ".agents/skills/research/SKILL.md";
fn markdown(body: &str) -> String {
    format!("---\nname: research\ndescription: Research notes\n---\n{body}\n")
}
fn seed() -> (tempfile::TempDir, Runtime, Arc<VaultService>) {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir_all(root.path().join(".agents/skills/research")).unwrap();
    fs::write(root.path().join(PATH), markdown("Old café 📝")).unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    (root, runtime, vault)
}

#[test]
fn detached_binding_keeps_immutable_instructions_and_independent_release() {
    let (root, _runtime, vault) = seed();
    let client = vault.attach().unwrap();
    let work = vault.begin_work().unwrap();
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    let context = SkillContext::default();
    let sources = SkillSources::default();
    let mut snapshot = work
        .capture_skill_binding(first, &context, &sources, || false)
        .unwrap();
    let original = snapshot.clone();
    snapshot.skills[0].name = "Caller mutation".into();
    drop(client);
    fs::write(root.path().join(PATH), markdown("New rules")).unwrap();
    work.invalidate_skill_catalog().unwrap();
    let retained = work
        .capture_skill_binding(first, &context, &sources, || false)
        .unwrap();
    assert_eq!(
        serde_json::to_value(retained).unwrap(),
        serde_json::to_value(original.clone()).unwrap()
    );
    let fresh = work
        .capture_skill_binding(second, &context, &sources, || false)
        .unwrap();
    assert_ne!(original.skills[0].version, fresh.skills[0].version);
    assert!(fresh.id.starts_with("skill-snapshot-1-"));
    assert_eq!(
        work.bound_skill(first, "research").unwrap().instructions,
        "Old café 📝"
    );
    assert_eq!(
        work.bound_skill(second, "research").unwrap().instructions,
        "New rules"
    );
    work.release_skill_binding(first).unwrap();
    assert_eq!(
        work.bound_skill(first, "research").unwrap_err().code(),
        ErrorCode::NotFound
    );
    assert_eq!(
        work.bound_skill(second, "research").unwrap().instructions,
        "New rules"
    );
    assert_eq!(vault.status().unwrap().clients, 0);
    assert_eq!(
        fs::read_to_string(root.path().join(PATH)).unwrap(),
        markdown("New rules")
    );
    assert_eq!(root.path().read_dir().unwrap().count(), 1);
}

#[test]
fn same_binding_capture_linearizes_and_foreign_vault_cannot_read_it() {
    let (_root, runtime, vault) = seed();
    let binding = Uuid::new_v4();
    let snapshots = thread::scope(|scope| {
        let a = vault.begin_work().unwrap();
        let b = vault.begin_work().unwrap();
        let run = |work: lapis_app_core::services::Work| {
            scope.spawn(move || {
                work.capture_skill_binding(
                    binding,
                    &SkillContext::default(),
                    &SkillSources::default(),
                    || false,
                )
                .unwrap()
            })
        };
        let first = run(a);
        let second = run(b);
        [first.join().unwrap(), second.join().unwrap()]
    });
    assert_eq!(snapshots[0].id, snapshots[1].id);
    let foreign = tempfile::tempdir().unwrap();
    let other = runtime.admit(Uuid::new_v4(), foreign.path()).unwrap();
    assert_eq!(
        other
            .begin_work()
            .unwrap()
            .bound_skill(binding, "research")
            .unwrap_err()
            .code(),
        ErrorCode::NotFound
    );
}

#[test]
fn cancel_before_listing_during_listing_or_after_read_never_installs_binding() {
    for threshold in [1, 4, 9] {
        let (root, _runtime, vault) = seed();
        let work = vault.begin_work().unwrap();
        let binding = Uuid::new_v4();
        let calls = Cell::new(0);
        let error = work
            .capture_skill_binding(
                binding,
                &SkillContext::default(),
                &SkillSources::default(),
                || {
                    calls.set(calls.get() + 1);
                    calls.get() == threshold
                },
            )
            .unwrap_err();
        assert_eq!(error.code(), ErrorCode::Closed);
        assert_eq!(calls.get(), threshold);
        assert_eq!(
            work.bound_skill(binding, "research").unwrap_err().code(),
            ErrorCode::NotFound
        );
        assert_eq!(vault.status().unwrap().work, 1);
        assert_eq!(
            fs::read_to_string(root.path().join(PATH)).unwrap(),
            markdown("Old café 📝")
        );
    }
}

#[test]
fn closing_fences_skill_preparation_and_historical_reads_until_work_drains() {
    let (_root, _runtime, vault) = seed();
    let work = vault.begin_work().unwrap();
    let binding = Uuid::new_v4();
    work.capture_skill_binding(
        binding,
        &SkillContext::default(),
        &SkillSources::default(),
        || false,
    )
    .unwrap();
    let (tx, rx) = mpsc::channel();
    thread::scope(|scope| {
        let closing = vault.clone();
        scope.spawn(move || tx.send(closing.close()).unwrap());
        let deadline = Instant::now() + Duration::from_secs(2);
        while vault.status().unwrap().phase == VaultPhase::Open {
            assert!(Instant::now() < deadline);
            thread::yield_now();
        }
        assert_eq!(
            work.capture_skill_binding(
                binding,
                &SkillContext::default(),
                &SkillSources::default(),
                || false
            )
            .unwrap_err()
            .code(),
            ErrorCode::Closed
        );
        assert_eq!(
            work.bound_skill(binding, "research").unwrap_err().code(),
            ErrorCode::Closed
        );
        assert!(rx.try_recv().is_err());
        drop(work);
        rx.recv_timeout(Duration::from_secs(2)).unwrap().unwrap();
    });
    assert_eq!(vault.status().unwrap().phase, VaultPhase::Closed);
}

#[test]
fn invalid_utf8_and_nonportable_scope_refuse_without_publishing_catalog() {
    let (root, _runtime, vault) = seed();
    let work = vault.begin_work().unwrap();
    fs::write(root.path().join(PATH), [0xff, 0xfe]).unwrap();
    let binding = Uuid::new_v4();
    assert_eq!(
        work.capture_skill_binding(
            binding,
            &SkillContext::default(),
            &SkillSources::default(),
            || false
        )
        .unwrap_err()
        .code(),
        ErrorCode::InvalidRequest
    );
    assert_eq!(
        work.bound_skill(binding, "research").unwrap_err().code(),
        ErrorCode::NotFound
    );
    for path in ["../outside", "/outside", "a/./b", "a//b", "C:x", "a\\b"] {
        let context = SkillContext {
            scope_dir: path.into(),
            ..Default::default()
        };
        assert_eq!(
            work.capture_skill_binding(Uuid::new_v4(), &context, &SkillSources::default(), || {
                false
            })
            .unwrap_err()
            .code(),
            ErrorCode::InvalidRequest
        );
    }
    assert_eq!(fs::read(root.path().join(PATH)).unwrap(), [0xff, 0xfe]);
}

#[test]
fn replaced_root_before_list_or_after_read_cannot_publish_new_instructions() {
    for threshold in [1, 9] {
        let (root, _runtime, vault) = seed();
        let work = vault.begin_work().unwrap();
        let retained = tempfile::tempdir().unwrap();
        let calls = Cell::new(0);
        let error = work
            .capture_skill_binding(
                Uuid::new_v4(),
                &SkillContext::default(),
                &SkillSources::default(),
                || {
                    calls.set(calls.get() + 1);
                    if calls.get() == threshold {
                        fs::rename(root.path(), retained.path().join("original")).unwrap();
                        fs::create_dir_all(root.path().join(".agents/skills/research")).unwrap();
                        fs::write(root.path().join(PATH), markdown("Replacement")).unwrap();
                    }
                    false
                },
            )
            .unwrap_err();
        assert_eq!(error.code(), ErrorCode::PermissionDenied);
        assert_eq!(
            fs::read_to_string(retained.path().join("original").join(PATH)).unwrap(),
            markdown("Old café 📝")
        );
        assert_eq!(
            fs::read_to_string(root.path().join(PATH)).unwrap(),
            markdown("Replacement")
        );
    }
}

#[test]
#[cfg(unix)]
fn linked_roots_are_denied_and_linked_skill_files_never_expand_authority() {
    use std::os::unix::fs::symlink;
    let (root, _runtime, vault) = seed();
    let work = vault.begin_work().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret"), markdown("Outside rules")).unwrap();
    fs::remove_file(root.path().join(PATH)).unwrap();
    symlink(outside.path().join("secret"), root.path().join(PATH)).unwrap();
    let value = work
        .capture_skill_binding(
            Uuid::new_v4(),
            &SkillContext::default(),
            &SkillSources::default(),
            || false,
        )
        .unwrap();
    assert!(value.skills.is_empty());
    symlink(outside.path(), root.path().join("escape")).unwrap();
    let sources = SkillSources {
        user_prefix: Some("escape".into()),
        ..Default::default()
    };
    assert_eq!(
        work.capture_skill_binding(Uuid::new_v4(), &SkillContext::default(), &sources, || false)
            .unwrap_err()
            .code(),
        ErrorCode::PermissionDenied
    );
    assert_eq!(
        fs::read_to_string(outside.path().join("secret")).unwrap(),
        markdown("Outside rules")
    );
}

#[test]
fn binding_retains_exact_bytes_even_when_a_source_reuses_a_version_label() {
    let root = tempfile::tempdir().unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let work = vault.begin_work().unwrap();
    let mut loaded = parse_skill(
        &markdown("First"),
        &SkillParseInput {
            path: "bundled/research/SKILL.md".into(),
            root: "bundled/research".into(),
            source: SkillSource::Bundled,
        },
    )
    .unwrap();
    loaded.version = "same-observed-label".into();
    let mut sources = SkillSources {
        bundled: vec![loaded],
        ..Default::default()
    };
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    work.capture_skill_binding(first, &SkillContext::default(), &sources, || false)
        .unwrap();
    sources.bundled[0].instructions = "Second".into();
    work.capture_skill_binding(second, &SkillContext::default(), &sources, || false)
        .unwrap();
    assert_eq!(
        work.bound_skill(first, "research").unwrap().instructions,
        "First"
    );
    assert_eq!(
        work.bound_skill(second, "research").unwrap().instructions,
        "Second"
    );
    assert_eq!(root.path().read_dir().unwrap().count(), 0);
}
