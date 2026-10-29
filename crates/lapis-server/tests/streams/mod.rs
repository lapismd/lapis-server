use super::support::Fixture;
use lapis_app_protocol::ClientDescriptor;

mod resources;
mod transfers;

fn url(f: &Fixture, client: &ClientDescriptor, path: &str) -> String {
    format!(
        "{}/api/v3/clients/{}/files/{path}",
        f.base, client.client_id
    )
}

#[tokio::test]
async fn large_file_read_uses_an_immutable_representation_and_matching_etag() {
    let f = Fixture::start().await;
    let client = f.attach(f.ids[0]).await;
    let bytes = vec![42; 6 * 1024 * 1024];
    std::fs::write(f.roots[0].path().join("large.bin"), &bytes).unwrap();
    let url = format!(
        "{}/api/v3/clients/{}/files/large.bin",
        f.base, client.client_id
    );
    let response = f.authorize(f.client.get(&url)).send().await.unwrap();
    assert_eq!(response.status(), 200);
    let etag = response.headers()["etag"].clone();
    assert_eq!(response.content_length(), Some(bytes.len() as u64));
    std::fs::write(f.roots[0].path().join("large.bin"), "external").unwrap();
    assert_eq!(response.bytes().await.unwrap().as_ref(), bytes);
    let changed = f.authorize(f.client.get(&url)).send().await.unwrap();
    assert_ne!(changed.headers()["etag"], etag);
    f.detach(&client).await;
    f.host.shutdown().unwrap();
}
