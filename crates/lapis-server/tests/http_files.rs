mod admission;
mod attachments;
mod backup;
mod browser_auth;
mod credentials;
mod database;
mod editing_recovery;
mod history_indexing;
mod history_reader;
mod indexing;
mod lifetime;
mod native_tasks;
mod physical;
mod repository_backup;
mod repository_restore;
mod repository_sources;
mod streams;
mod support;

use lapis_app_protocol::{
    ErrorCode, Outcome, VaultCommand, VaultValue,
    files::{FileCommand, FileOptions, FileValue},
};
use support::Fixture;

fn file(operation: FileCommand) -> VaultCommand {
    VaultCommand::Files { operation }
}
fn value(reply: lapis_app_protocol::Reply) -> FileValue {
    match reply.outcome {
        Outcome::Ok {
            value: VaultValue::Files { value },
        } => value,
        other => panic!("unexpected outcome: {other:?}"),
    }
}

#[tokio::test]
async fn original_native_file_operations_use_the_shared_owner_over_real_http() {
    let f = Fixture::start().await;
    let first = f.attach(f.ids[0]).await;
    let second = f.attach(f.ids[0]).await;
    assert_eq!(first.generation, second.generation);
    let run = |command| {
        let f = &f;
        let first = &first;
        async move {
            let reply = f.execute(first, file(command)).await;
            assert!(matches!(&reply.outcome, Outcome::Ok { .. }), "{reply:?}");
            reply
        }
    };
    assert!(matches!(
        value(
            run(FileCommand::Mkdir {
                path: ".lapis".into(),
                recursive: true
            })
            .await
        ),
        FileValue::Done {}
    ));
    run(FileCommand::WriteText {
        path: ".lapis/app.json".into(),
        data: "{}".into(),
        options: FileOptions::default(),
    })
    .await;
    run(FileCommand::CreateFile {
        path: "é.md".into(),
        data: "# 💎\n".as_bytes().to_vec(),
        options: FileOptions::default(),
    })
    .await;
    let read = f
        .execute(
            &second,
            file(FileCommand::ReadText {
                path: "é.md".into(),
            }),
        )
        .await;
    assert!(matches!(value(read), FileValue::Text { data } if data == "# 💎\n"));
    run(FileCommand::AppendText {
        path: "é.md".into(),
        data: "next".into(),
        options: FileOptions::default(),
    })
    .await;
    run(FileCommand::WriteTextIfUnchanged {
        path: "é.md".into(),
        expected: Some("# 💎\nnext".into()),
        data: "saved".into(),
        options: FileOptions {
            mtime: Some(1700000000000.0),
        },
    })
    .await;
    let stat = value(
        run(FileCommand::Stat {
            path: "é.md".into(),
        })
        .await,
    );
    assert!(
        matches!(stat, FileValue::Stat { stat } if stat.as_ref().is_some_and(|stat| stat.mtime == 1700000000000.0))
    );
    run(FileCommand::WriteBinary {
        path: "data.bin".into(),
        data: vec![0, 255, 128, 42],
        options: FileOptions::default(),
    })
    .await;
    assert!(
        matches!(value(run(FileCommand::ReadBinary { path: "data.bin".into() }).await), FileValue::Binary { data } if data == vec![0,255,128,42])
    );
    assert!(
        matches!(value(run(FileCommand::ReadRange { path: "data.bin".into(), offset: 1, length: 2 }).await), FileValue::Binary { data } if data == vec![255,128])
    );
    run(FileCommand::Copy {
        path: "é.md".into(),
        new_path: "copy.md".into(),
    })
    .await;
    run(FileCommand::Rename {
        path: "copy.md".into(),
        new_path: "renamed.md".into(),
    })
    .await;
    assert!(matches!(
        value(
            run(FileCommand::Exists {
                path: "renamed.md".into()
            })
            .await
        ),
        FileValue::Exists { exists: true }
    ));
    assert!(
        matches!(value(run(FileCommand::List { path: "".into() }).await), FileValue::List { listing } if listing.files.contains(&"renamed.md".into()))
    );
    let collision = f
        .command(
            &first,
            file(FileCommand::CreateFile {
                path: "é.md".into(),
                data: vec![],
                options: FileOptions::default(),
            }),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(collision.status(), 409);
    let reply: lapis_app_protocol::Reply = collision.json().await.unwrap();
    assert!(
        matches!(reply.outcome, Outcome::Error { error } if error.code == ErrorCode::AlreadyExists)
    );
    std::fs::write(f.roots[0].path().join("é.md"), "external").unwrap();
    let conflict = f
        .execute(
            &first,
            file(FileCommand::WriteTextIfUnchanged {
                path: "é.md".into(),
                expected: Some("saved".into()),
                data: "unsafe".into(),
                options: FileOptions::default(),
            }),
        )
        .await;
    assert!(
        matches!(conflict.outcome, Outcome::Error { error } if error.code == ErrorCode::Conflict)
    );
    assert_eq!(
        std::fs::read_to_string(f.roots[0].path().join("é.md")).unwrap(),
        "external"
    );
    run(FileCommand::Remove {
        path: "renamed.md".into(),
    })
    .await;
    run(FileCommand::Rmdir {
        path: ".lapis".into(),
        recursive: true,
    })
    .await;
    f.detach(&first).await;
    assert!(
        matches!(value(f.execute(&second, file(FileCommand::ReadText { path: "é.md".into() })).await), FileValue::Text { data } if data == "external")
    );
    assert!(!f.roots[1].path().join("é.md").exists());
    f.detach(&second).await;
    assert_eq!(f.host.vaults().unwrap().len(), 2);
    f.host.shutdown().unwrap();
}
