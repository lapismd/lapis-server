use super::support::Fixture;
use serde_json::json;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remotes_routes_exist_and_refuse_a_profile_without_native_custody() {
    let f = Fixture::with_database().await;
    let c = f.attach(f.ids[0]).await;
    let url = format!(
        "{}/api/v3/clients/{}/repository-remotes?vault={}&generation={}",
        f.base, c.client_id, c.vault_id, c.generation
    );
    let response = f.authorize(f.client.get(&url)).send().await.unwrap();
    assert_eq!(response.status(), 501);
    let response = f
        .authorize(f.client.post(&url))
        .json(&json!({"kind":"fetch", "target":"backup", "expectedRevision":0}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 501);
}
