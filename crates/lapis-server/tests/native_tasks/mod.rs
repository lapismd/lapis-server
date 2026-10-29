mod en_gb;
use crate::support::Fixture;
use serde_json::{Value, json};

fn route(f: &Fixture, client: uuid::Uuid) -> String {
    format!("{}/api/v3/clients/{client}/tasks", f.base)
}

#[tokio::test]
async fn native_tasks_route_keeps_file_only_profiles_explicitly_unavailable() {
    let f = Fixture::start().await;
    let client = f.attach(f.ids[0]).await;
    let response = f
        .authorize(f.client.post(route(&f, client.client_id)))
        .json(&json!({"kind":"status"}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 501);
    assert_eq!(
        response.json::<Value>().await.unwrap()["code"],
        "UNSUPPORTED"
    );
    f.host.shutdown().unwrap();
}

#[tokio::test]
async fn native_tasks_route_uses_existing_authority_and_rejects_callback_or_path_inputs() {
    let f = Fixture::start().await;
    let client = f.attach(f.ids[0]).await;
    let url = route(&f, client.client_id);
    let status = json!({"kind":"status"});
    let unauthorized = f
        .client
        .post(&url)
        .header("x-lapis-protocol", "3")
        .json(&status)
        .send()
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), 401);
    let foreign = f
        .authorize(f.client.post(&url))
        .header("origin", "https://foreign.invalid")
        .json(&status)
        .send()
        .await
        .unwrap();
    assert_eq!(foreign.status(), 403);
    for command in [
        json!({"kind":"step"}),
        json!({"kind":"status","root":"/outside"}),
        json!({"kind":"status","profile":"/outside"}),
        json!({"kind":"status","caller":"other"}),
        json!({"kind":"status","callback":"write"}),
    ] {
        let rejected = f
            .authorize(f.client.post(&url))
            .json(&command)
            .send()
            .await
            .unwrap();
        assert_eq!(rejected.status(), 422);
        assert_eq!(
            rejected.json::<Value>().await.unwrap()["code"],
            "INVALID_REQUEST"
        );
    }
    f.host.shutdown().unwrap();
}

#[tokio::test]
async fn native_tasks_route_fences_retired_clients_and_preserves_peer_admission() {
    let f = Fixture::start().await;
    let old = f.attach(f.ids[0]).await;
    let peer = f.attach(f.ids[1]).await;
    f.detach(&old).await;
    let rejected = f
        .authorize(f.client.post(route(&f, old.client_id)))
        .json(&json!({"kind":"status"}))
        .send()
        .await
        .unwrap();
    assert_eq!(rejected.status(), 403);
    let available = f
        .authorize(f.client.post(route(&f, peer.client_id)))
        .json(&json!({"kind":"status"}))
        .send()
        .await
        .unwrap();
    assert_eq!(available.status(), 501);
    f.host.shutdown().unwrap();
}

#[tokio::test]
async fn native_tasks_selected_profile_keeps_guarded_mutations_and_journal_inspection_on_one_client()
 {
    use lapis_app_protocol::{Outcome, VaultCommand, VaultValue};
    let f = Fixture::with_database().await;
    let path = "Task.md";
    let raw = "---\ntype: task\nid: read\ntask:\n  status: open\n  inbox: false\n  start: anytime\n---\n\n# Read\n";
    std::fs::write(f.roots[0].path().join(path), raw).unwrap();
    let attached = f.attach(f.ids[0]).await;
    let url = route(&f, attached.client_id);
    let call = |command: Value| {
        let f = &f;
        let url = &url;
        async move {
            let response = f
                .authorize(f.client.post(url))
                .json(&command)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), 200);
            assert_eq!(response.headers()["cache-control"], "no-store");
            response.json::<Value>().await.unwrap()
        }
    };
    assert_eq!(
        call(json!({"kind":"status"})).await["status"]["enabled"],
        false
    );
    call(json!({"kind":"configure","configuration":{"day_parts":{"afternoonStartsAt":"12:00","eveningStartsAt":"17:00"},"daily":{"folder":"daily","date_format":"yyyy-LL-dd"},"format":{"locale":"en-US","firstDay":7,"minimalDays":4},"provider":{"kind":"default"}}})).await;
    let Outcome::Ok {
        value: VaultValue::Text { snapshot },
    } = f
        .execute(&attached, VaultCommand::ReadText { path: path.into() })
        .await
        .outcome
    else {
        panic!("read")
    };
    let request = json!({"request_id":uuid::Uuid::new_v4(),"path":path,"expected_version":snapshot.version,"action":{"type":"complete","at":"2026-10-26T09:00:00Z","localDate":"2026-10-26","trackedValue":null}});
    let result = call(json!({"kind":"submit","request":request})).await;
    assert_eq!(result["operation"]["disposition"], "completed");
    let after = std::fs::read_to_string(f.roots[0].path().join(path)).unwrap();
    assert!(after.contains("status: completed"));
    assert_eq!(after, result["operation"]["task_replacement"]);
    assert_eq!(
        call(json!({"kind":"inspect","request_id":request["request_id"]})).await,
        result
    );
    assert_eq!(
        call(json!({"kind":"submit","request":request})).await,
        result
    );
    assert_eq!(
        std::fs::read_to_string(f.roots[0].path().join(path)).unwrap(),
        after
    );
    let host = f.host.clone();
    tokio::task::spawn_blocking(move || host.shutdown())
        .await
        .unwrap()
        .unwrap();
}
