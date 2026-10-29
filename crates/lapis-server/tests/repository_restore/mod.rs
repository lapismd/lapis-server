use super::support::{Fixture, TOKEN};
use jj_store_jj::vault_workspace::{Binding, HistoryReader};
use lapis_app_protocol::{ErrorCode, Outcome, ServiceError, VaultCommand, VaultValue};
use serde_json::{Value, json};
use std::fs;
use uuid::Uuid;

fn route(f: &Fixture, client: Uuid) -> String {
    format!("{}/api/v3/clients/{client}/repository-restore", f.base)
}
async fn invoke(f: &Fixture, client: Uuid, input: Value) -> reqwest::Response {
    f.authorize(f.client.post(route(f, client)))
        .json(&input)
        .send()
        .await
        .unwrap()
}
async fn success(f: &Fixture, client: Uuid, input: Value) -> Value {
    invoke(f, client, input)
        .await
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}
async fn checkpoint(f: &Fixture, index: usize) -> String {
    let binding = Binding {
        vault_id: f.ids[index].to_string(),
        root: f.roots[index].path().canonicalize().unwrap(),
        state: f
            .profile
            .as_ref()
            .unwrap()
            .path()
            .canonicalize()
            .unwrap()
            .join("vault-history")
            .join(f.ids[index].to_string()),
    };
    HistoryReader::open(binding)
        .await
        .unwrap()
        .history_revision(None, 0)
        .await
        .unwrap()
        .revision
        .unwrap()
        .checkpoint
        .commit
}
fn request(checkpoint: &str, expected: &str) -> Value {
    json!({"request_id":Uuid::new_v4(),"checkpoint":checkpoint,"path":"Note.md","expected":expected})
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn file_restore_uses_expected_saved_bytes_and_retains_exact_retry_receipts() {
    let f = Fixture::with_database().await;
    let path = f.roots[0].path().join("Note.md");
    fs::write(&path, "first saved version\n").unwrap();
    let c = f.attach(f.ids[0]).await;
    let first = checkpoint(&f, 0).await;
    fs::write(&path, "second saved version\n").unwrap();
    success(&f, c.client_id, json!({"kind":"keep_current"})).await;
    let preview = success(
        &f,
        c.client_id,
        json!({"kind":"preview","checkpoint":first,"path":"Note.md"}),
    )
    .await;
    assert_eq!(preview["checkpoint"], first);
    assert_eq!(preview["bytes"], 20);
    assert_eq!(preview["deleted"], false);
    let apply =
        json!({"kind":"apply","request":request(&first, preview["expected"].as_str().unwrap())});
    fs::write(&path, "external change after preview\n").unwrap();
    let rejected = invoke(&f, c.client_id, apply).await;
    assert_eq!(rejected.status(), 409);
    let error = rejected.json::<ServiceError>().await.unwrap();
    assert_eq!(error.code, ErrorCode::Conflict);
    assert_eq!(
        error.message,
        "The file changed since the restore preview; review it again"
    );
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "external change after preview\n"
    );
    let preview = success(
        &f,
        c.client_id,
        json!({"kind":"preview","checkpoint":first,"path":"Note.md"}),
    )
    .await;
    let apply =
        json!({"kind":"apply","request":request(&first, preview["expected"].as_str().unwrap())});
    let receipt = success(&f, c.client_id, apply.clone()).await;
    assert_eq!(fs::read_to_string(&path).unwrap(), "first saved version\n");
    fs::write(&path, "saved after completed restore\n").unwrap();
    assert_eq!(success(&f, c.client_id, apply).await, receipt);
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "saved after completed restore\n"
    );
    assert!(!f.roots[0].path().join(".jj").exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn vault_restore_reuses_the_native_preview_and_publication_engine() {
    let f = Fixture::with_database().await;
    let root = f.roots[0].path();
    fs::write(root.join("Note.md"), "retained checkpoint\n").unwrap();
    let c = f.attach(f.ids[0]).await;
    let first = checkpoint(&f, 0).await;
    fs::remove_file(root.join("Note.md")).unwrap();
    fs::write(root.join("Later.md"), "later file\n").unwrap();
    success(&f, c.client_id, json!({"kind":"keep_current"})).await;
    let preview = success(
        &f,
        c.client_id,
        json!({"kind":"preview_vault","checkpoint":first}),
    )
    .await;
    assert_eq!(preview["added"], 1);
    assert_eq!(preview["deleted"], 1);
    let apply = json!({"kind":"apply_vault","request":{
        "request_id":Uuid::new_v4(),"checkpoint":first,"expected":preview["expected"]
    }});
    let receipt = success(&f, c.client_id, apply.clone()).await;
    assert_eq!(
        fs::read_to_string(root.join("Note.md")).unwrap(),
        "retained checkpoint\n"
    );
    assert!(!root.join("Later.md").exists());
    assert_eq!(success(&f, c.client_id, apply).await, receipt);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restore_refuses_detached_unresolved_editor_claims_without_changing_saved_files() {
    let f = Fixture::with_database().await;
    let path = f.roots[0].path().join("Note.md");
    fs::write(&path, "first saved version\n").unwrap();
    let c = f.attach(f.ids[0]).await;
    let first = checkpoint(&f, 0).await;
    fs::write(&path, "current saved version\n").unwrap();
    let owner = f.attach(f.ids[0]).await;
    let preview = success(
        &f,
        owner.client_id,
        json!({"kind":"preview","checkpoint":first,"path":"Note.md"}),
    )
    .await;
    let editor = match f
        .execute(
            &c,
            VaultCommand::OpenEditor {
                path: "Note.md".into(),
            },
        )
        .await
        .outcome
    {
        Outcome::Ok {
            value: VaultValue::Editor { editor_id },
        } => editor_id,
        other => panic!("{other:?}"),
    };
    f.execute(
        &c,
        VaultCommand::MarkDirty {
            editor_id: editor,
            revision: 1,
        },
    )
    .await;
    f.detach(&c).await;
    let rejected = invoke(
        &f,
        owner.client_id,
        json!({"kind":"apply","request":request(&first,preview["expected"].as_str().unwrap())}),
    )
    .await;
    assert_eq!(rejected.status(), 409);
    let error = rejected.json::<ServiceError>().await.unwrap();
    assert_eq!(error.code, ErrorCode::Busy);
    assert_eq!(error.message, "Vault has unresolved editor state");
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        "current saved version\n"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restore_is_bounded_authenticated_and_never_accepts_foreign_authority() {
    let f = Fixture::with_database().await;
    let c = f.attach(f.ids[0]).await;
    let url = route(&f, c.client_id);
    let input = json!({"kind":"keep_current"});
    assert_eq!(
        f.client
            .post(&url)
            .json(&input)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        f.client
            .post(&url)
            .bearer_auth(TOKEN)
            .json(&input)
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    assert_eq!(
        f.authorize(f.client.post(&url))
            .header("origin", "https://foreign.invalid")
            .json(&input)
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    for (input, status) in [
        (json!({"kind":"capture"}), 422),
        (json!({"kind":"keep_current","root":"/outside"}), 422),
        (
            json!({"kind":"preview","checkpoint":"","path":"Note.md"}),
            400,
        ),
        (
            json!({"kind":"preview","checkpoint":"abc","path":"../Note.md"}),
            400,
        ),
        (
            json!({"kind":"apply","request":{"request_id":"","checkpoint":"abc","expected":"def","path":"Note.md"}}),
            400,
        ),
    ] {
        assert_eq!(invoke(&f, c.client_id, input).await.status(), status);
    }
    assert_eq!(
        invoke(
            &f,
            c.client_id,
            json!({"kind":"preview","checkpoint":"a".repeat(20_000),"path":"Note.md"})
        )
        .await
        .status(),
        413
    );
    let foreign = f
        .host
        .attach(
            f.ids[1],
            lapis_server::host::Principal::Session(Uuid::new_v4()),
        )
        .unwrap();
    assert_eq!(
        invoke(&f, foreign.client_id, json!({"kind":"keep_current"}))
            .await
            .status(),
        403
    );
    fs::write(
        f.roots[1].path().join("Other.md"),
        "foreign vault checkpoint",
    )
    .unwrap();
    let other = f.attach(f.ids[1]).await;
    let foreign_checkpoint = checkpoint(&f, 1).await;
    assert_eq!(
        invoke(
            &f,
            c.client_id,
            json!({"kind":"preview_vault","checkpoint":foreign_checkpoint})
        )
        .await
        .status(),
        409
    );
    assert_eq!(
        fs::read_to_string(f.roots[1].path().join("Other.md")).unwrap(),
        "foreign vault checkpoint"
    );
    f.detach(&other).await;
    f.detach(&c).await;
    assert_eq!(invoke(&f, c.client_id, input).await.status(), 403);
}
