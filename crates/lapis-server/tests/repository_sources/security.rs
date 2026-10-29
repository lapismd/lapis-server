use super::*;
use crate::support::TOKEN;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn source_discovery_is_bounded_authenticated_and_cannot_receive_authority_overrides() {
    let f = Fixture::with_database().await;
    init(f.roots[0].path());
    let client = f.attach(f.ids[0]).await;
    let url = format!(
        "{}/api/v3/clients/{}/repository-sources",
        f.base, client.client_id
    );
    assert_eq!(
        f.client
            .post(&url)
            .json(&json!({"kind":"discover"}))
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
            .json(&json!({"kind":"discover"}))
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    assert_eq!(
        f.authorize(f.client.post(&url))
            .header("origin", "https://foreign.invalid")
            .json(&json!({"kind":"discover"}))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    for key in [
        "root",
        "metadata",
        "source",
        "token",
        "window",
        "generation",
        "vault",
    ] {
        let mut value = json!({"kind":"open_original"});
        value[key] = json!("outside");
        assert_eq!(
            source_response(&f, client.client_id, value).await.status(),
            422
        );
    }
    assert_eq!(
        source_response(
            &f,
            client.client_id,
            json!({"kind":"open_original","token":"a".repeat(20_000)})
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
        source_response(&f, foreign.client_id, json!({"kind":"discover"}))
            .await
            .status(),
        403
    );
    f.detach(&client).await;
    assert_eq!(
        source_response(&f, client.client_id, json!({"kind":"discover"}))
            .await
            .status(),
        403
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn folders_and_git_markers_do_not_initialize_or_fall_back_to_an_original_source() {
    for git in [false, true] {
        let f = Fixture::with_database().await;
        let root = f.roots[0].path();
        if git {
            fs::create_dir(root.join(".git")).unwrap();
        }
        let before = metadata(root);
        let client = f.attach(f.ids[0]).await;
        assert_eq!(
            sources(&f, client.client_id, "discover").await,
            json!({"original":false,"token":null})
        );
        assert!(
            !source_response(&f, client.client_id, json!({"kind":"open_original"}))
                .await
                .status()
                .is_success()
        );
        assert!(!root.join(".jj").exists());
        assert_eq!(metadata(root), before);
        f.detach(&client).await;
    }
}
