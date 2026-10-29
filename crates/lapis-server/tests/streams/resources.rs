use super::{Fixture, url};

#[tokio::test]
async fn resources_retain_mime_sandbox_range_and_vault_admission() {
    let f = Fixture::start().await;
    let client = f.attach(f.ids[0]).await;
    std::fs::write(f.roots[0].path().join("image.svg"), "<svg>image</svg>").unwrap();
    std::fs::write(f.roots[0].path().join("note.md"), "private note").unwrap();
    let resource = url(&f, &client, "image.svg").replace("/files/", "/resources/");
    assert_eq!(f.client.get(&resource).send().await.unwrap().status(), 401);
    let response = f
        .authorize(f.client.get(&resource))
        .header("range", "bytes=0-4")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 206);
    assert_eq!(response.headers()["content-type"], "image/svg+xml");
    assert_eq!(
        response.headers()["content-security-policy"],
        "default-src 'none'; sandbox"
    );
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert_eq!(response.text().await.unwrap(), "<svg>");
    let note = url(&f, &client, "note.md").replace("/files/", "/resources/");
    assert_eq!(
        f.authorize(f.client.get(note))
            .send()
            .await
            .unwrap()
            .status(),
        415
    );
    let invalid = url(&f, &client, "%2E%2E%2Foutside.md");
    assert_eq!(
        f.authorize(f.client.get(invalid))
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    f.detach(&client).await;
    assert_eq!(
        f.authorize(f.client.get(&resource))
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    f.host.shutdown().unwrap();
}
