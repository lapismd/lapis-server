use super::support::Fixture;
use lapis_app_protocol::{ExpectedVersion, VaultCommand};

#[cfg(unix)]
#[tokio::test]
async fn symlink_escapes_and_replaced_roots_never_reopen_an_ambient_path() {
    let f = Fixture::start().await;
    let client = f.attach(f.ids[0]).await;
    let outside = f.roots[1].path().join("private.md");
    std::fs::write(&outside, "private").unwrap();
    std::os::unix::fs::symlink(&outside, f.roots[0].path().join("escape.md")).unwrap();
    let response = f
        .command(
            &client,
            VaultCommand::ReadText {
                path: "escape.md".into(),
            },
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
    let text = response.text().await.unwrap();
    assert!(!text.contains(f.roots[1].path().to_str().unwrap()));
    let moved = tempfile::tempdir().unwrap();
    std::fs::rename(f.roots[0].path(), moved.path().join("original")).unwrap();
    std::fs::create_dir(f.roots[0].path()).unwrap();
    let response = f
        .command(
            &client,
            VaultCommand::WriteText {
                path: "unsafe.md".into(),
                text: "unsafe".into(),
                expected: ExpectedVersion::Missing {},
            },
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 403);
    assert!(!f.roots[0].path().join("unsafe.md").exists());
    assert_eq!(std::fs::read_to_string(outside).unwrap(), "private");
    f.detach(&client).await;
    f.host.shutdown().unwrap();
}
