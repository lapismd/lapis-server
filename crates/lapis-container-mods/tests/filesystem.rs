use cap_std::{ambient_authority, fs::Dir};
use lapis_container_mods::{
    archive::{self, Kind},
    config::Config,
    install,
};
use std::{
    fs,
    io::Cursor,
    os::unix::fs::{PermissionsExt, symlink},
    path::Path,
};

fn layer(root: &Path, entries: &[(&str, &[u8], u32, Option<&str>)]) -> std::path::PathBuf {
    let path = root.join("layer.tar");
    let mut tar = tar::Builder::new(fs::File::create(&path).unwrap());
    for (name, bytes, mode, link) in entries {
        let mut header = tar::Header::new_ustar();
        header.set_mode(*mode);
        // SAFETY: identity queries have no preconditions.
        header.set_uid(unsafe { libc::geteuid() }.into());
        header.set_gid(unsafe { libc::getegid() }.into());
        if let Some(target) = link {
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_link_name(target).unwrap();
            header.set_size(0);
        } else {
            header.set_entry_type(tar::EntryType::Regular);
            header.set_size(bytes.len() as u64);
        }
        header.set_cksum();
        tar.append_data(&mut header, name, Cursor::new(bytes))
            .unwrap();
    }
    tar.finish().unwrap();
    path
}
const RAW: &str = "application/vnd.oci.image.layer.v1.tar";
#[test]
fn no_mods_and_configuration_are_conservative_and_not_shell_evaluated() {
    let config = Config::parse(|_| None).unwrap();
    assert!(config.references.is_empty());
    assert!(config.strict);
    assert!(Config::parse(|key| (key == "DOCKER_MODS").then(|| "ghcr.io/lapis/a:v1|docker.io/library/b@sha256:0000000000000000000000000000000000000000000000000000000000000000".into())).unwrap().references.len() == 2);
    for value in ["a||b", "$(touch /tmp/oops)", "a|", "a|a"] {
        assert!(Config::parse(|key| (key == "DOCKER_MODS").then(|| value.into())).is_err());
    }
    assert!(Config::parse(|key| (key == "DOCKER_MODS_STRICT").then(|| "yes;true".into())).is_err());
}
#[test]
fn executable_ownership_and_relative_symlink_survive_application() {
    let temp = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let blob = layer(
        temp.path(),
        &[
            (
                "usr/local/bin/hello",
                b"#!/bin/sh\necho hello\n",
                0o755,
                None,
            ),
            ("usr/local/bin/hi", b"", 0o777, Some("hello")),
        ],
    );
    let plan = archive::prepare(&blob, RAW, temp.path()).unwrap();
    install::apply(
        &Dir::open_ambient_dir(root.path(), ambient_authority()).unwrap(),
        &plan,
    )
    .unwrap();
    assert_eq!(
        fs::read(root.path().join("usr/local/bin/hi")).unwrap(),
        b"#!/bin/sh\necho hello\n"
    );
    assert_eq!(
        fs::metadata(root.path().join("usr/local/bin/hello"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
    assert!(matches!(plan.entries[1].kind, Kind::Symlink(_)));
}
#[test]
fn collisions_preserve_original_files_and_explicit_replacement_is_ordered() {
    let temp = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("config"), "original").unwrap();
    let blob = layer(
        temp.path(),
        &[
            ("new", b"new", 0o644, None),
            ("config", b"replacement", 0o644, None),
        ],
    );
    let plan = archive::prepare(&blob, RAW, temp.path()).unwrap();
    let dir = Dir::open_ambient_dir(root.path(), ambient_authority()).unwrap();
    assert!(install::apply(&dir, &plan).is_err());
    assert!(!root.path().join("new").exists());
    assert_eq!(
        fs::read_to_string(root.path().join("config")).unwrap(),
        "original"
    );
    let blob = layer(
        temp.path(),
        &[
            ("config", b"replacement", 0o644, None),
            (
                "opt/mods/example/mod.json",
                br#"{"replace":["config"]}"#,
                0o644,
                None,
            ),
        ],
    );
    install::apply(&dir, &archive::prepare(&blob, RAW, temp.path()).unwrap()).unwrap();
    assert_eq!(
        fs::read_to_string(root.path().join("config")).unwrap(),
        "replacement"
    );
}
#[test]
fn unsafe_archives_and_existing_symlink_parents_are_rejected_before_writes() {
    let temp = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    for (name, mode, target) in [
        (".wh.hidden", 0o644, None),
        ("tool", 0o4755, None),
        ("link", 0o777, Some("../../escape")),
        ("link", 0o777, Some("/etc/passwd")),
    ] {
        let blob = layer(temp.path(), &[(name, b"x", mode, target)]);
        assert!(archive::prepare(&blob, RAW, temp.path()).is_err());
    }
    let blob = layer(
        temp.path(),
        &[
            ("link", b"", 0o777, Some("safe")),
            ("link/child", b"x", 0o644, None),
        ],
    );
    assert!(archive::prepare(&blob, RAW, temp.path()).is_err());
    let outside = tempfile::tempdir().unwrap();
    symlink(outside.path(), root.path().join("link")).unwrap();
    let blob = layer(temp.path(), &[("link/child", b"x", 0o644, None)]);
    let plan = archive::prepare(&blob, RAW, temp.path()).unwrap();
    assert!(
        install::apply(
            &Dir::open_ambient_dir(root.path(), ambient_authority()).unwrap(),
            &plan
        )
        .is_err()
    );
    assert!(!outside.path().join("child").exists());
    assert!(archive::relative(Path::new("../escape")).is_err());
    assert!(archive::relative(Path::new("/absolute")).is_err());
}
#[test]
fn malformed_layers_compression_metadata_and_hook_order_are_checked() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("bad");
    fs::write(&path, "not a tar archive").unwrap();
    assert!(archive::prepare(&path, RAW, temp.path()).is_err());
    assert!(archive::prepare(&path, "zstd", temp.path()).is_err());
    let blob = layer(
        temp.path(),
        &[
            ("opt/mods/z/post-init.sh", b"exit 0", 0o755, None),
            ("opt/mods/a/init.sh", b"exit 0", 0o755, None),
        ],
    );
    let plan = archive::prepare(&blob, RAW, temp.path()).unwrap();
    assert_eq!(plan.hooks[0], Path::new("opt/mods/a/init.sh"));
    let blob = layer(
        temp.path(),
        &[("opt/mods/a/init.sh", b"exit 0", 0o644, None)],
    );
    assert!(archive::prepare(&blob, RAW, temp.path()).is_err());
}

#[test]
fn configured_registry_credentials_are_read_without_trusting_app_writable_helpers() {
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("config.json");
    fs::write(&file, r#"{"auths":{"ghcr.io":{"auth":"dXNlcjpwYXNz"}}}"#).unwrap();
    let reference = "ghcr.io/example/mod:version".parse().unwrap();
    assert!(
        matches!(lapis_container_mods::credentials::from_file(&file, &reference).unwrap(), oci_client::secrets::RegistryAuth::Basic(user, password) if user == "user" && password == "pass")
    );
    fs::set_permissions(&file, fs::Permissions::from_mode(0o666)).unwrap();
    assert!(lapis_container_mods::credentials::from_file(&file, &reference).is_err());
}

#[test]
fn failed_publication_rolls_back_replaced_file_bytes_and_permissions() {
    let temp = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("config"), "original").unwrap();
    fs::set_permissions(
        root.path().join("config"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let blob = layer(
        temp.path(),
        &[
            ("config", b"replacement", 0o644, None),
            ("later", b"later", 0o644, None),
            (
                "opt/mods/test/mod.json",
                br#"{"replace":["config"]}"#,
                0o644,
                None,
            ),
        ],
    );
    let mut plan = archive::prepare(&blob, RAW, temp.path()).unwrap();
    // Inject an OS publication failure after the first atomic replacement.
    plan.entries[1].kind = Kind::Symlink(std::path::PathBuf::from("invalid\0target"));
    assert!(
        install::apply(
            &Dir::open_ambient_dir(root.path(), ambient_authority()).unwrap(),
            &plan
        )
        .is_err()
    );
    assert_eq!(
        fs::read_to_string(root.path().join("config")).unwrap(),
        "original"
    );
    assert_eq!(
        fs::metadata(root.path().join("config"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert!(!root.path().join("later").exists());
}

#[test]
fn symlink_chains_cannot_escape_through_archive_or_existing_links() {
    let temp = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    symlink(outside.path(), root.path().join("alias")).unwrap();
    let blob = layer(
        temp.path(),
        &[(
            "usr/local/bin/new-link",
            b"",
            0o777,
            Some("../../../alias/file"),
        )],
    );
    let plan = archive::prepare(&blob, RAW, temp.path()).unwrap();
    let dir = Dir::open_ambient_dir(root.path(), ambient_authority()).unwrap();
    assert!(install::apply(&dir, &plan).is_err());
    assert!(!root.path().join("usr/local/bin/new-link").exists());
    let blob = layer(
        temp.path(),
        &[
            ("directory-link", b"", 0o777, Some(".")),
            ("escape", b"", 0o777, Some("directory-link/../outside")),
        ],
    );
    let plan = archive::prepare(&blob, RAW, temp.path()).unwrap();
    assert!(install::apply(&dir, &plan).is_err());
    let blob = layer(
        temp.path(),
        &[
            ("first", b"", 0o777, Some("second")),
            ("second", b"", 0o777, Some("tool")),
            ("tool", b"safe", 0o644, None),
        ],
    );
    install::apply(&dir, &archive::prepare(&blob, RAW, temp.path()).unwrap()).unwrap();
    assert_eq!(
        fs::read_to_string(root.path().join("first")).unwrap(),
        "safe"
    );
}
