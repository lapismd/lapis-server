use super::*;

#[tokio::test]
async fn scoped_tokens_are_required_and_revocation_precedes_query_validation() {
    let f = Fixture::start().await;
    for builder in [
        f.client
            .get(format!("{}/api/content/v1/vaults?root=/private", f.base))
            .header("cookie", "lapis-session=owner"),
        f.client
            .get(format!("{}/api/content/v1/capabilities", f.base))
            .bearer_auth("owner-token"),
    ] {
        error(
            builder.send().await.unwrap(),
            StatusCode::UNAUTHORIZED,
            ContentErrorCode::Unauthenticated,
        )
        .await;
    }
    f.grants.revoke(f.grant).unwrap();
    error(
        f.get("/vaults?limit=invalid").send().await.unwrap(),
        StatusCode::UNAUTHORIZED,
        ContentErrorCode::Unauthenticated,
    )
    .await;
}

#[tokio::test]
async fn authority_origin_protocol_and_unimplemented_methods_are_refused() {
    let f = Fixture::start().await;
    error(
        f.get("/capabilities")
            .header("host", "attacker.example")
            .send()
            .await
            .unwrap(),
        StatusCode::FORBIDDEN,
        ContentErrorCode::PermissionDenied,
    )
    .await;
    error(
        f.get("/capabilities")
            .header("origin", "http://attacker.example")
            .send()
            .await
            .unwrap(),
        StatusCode::FORBIDDEN,
        ContentErrorCode::PermissionDenied,
    )
    .await;
    value(
        f.get("/capabilities")
            .header("origin", &f.base)
            .send()
            .await
            .unwrap(),
    )
    .await;
    error(
        f.client
            .get(format!("{}/api/content/v1/capabilities", f.base))
            .bearer_auth(&f.token)
            .header(API_HEADER, "3")
            .send()
            .await
            .unwrap(),
        StatusCode::CONFLICT,
        ContentErrorCode::ProtocolMismatch,
    )
    .await;
    error(
        f.authorize(
            f.client
                .post(format!("{}/api/content/v1{}", f.base, f.note_path())),
        )
        .body("must not write")
        .send()
        .await
        .unwrap(),
        StatusCode::METHOD_NOT_ALLOWED,
        ContentErrorCode::Unsupported,
    )
    .await;
    assert!(!f.root.path().join("note.md").exists());
    error(
        f.get("/grants").send().await.unwrap(),
        StatusCode::NOT_FOUND,
        ContentErrorCode::NotFound,
    )
    .await;
}

#[tokio::test]
async fn malformed_queries_and_oversized_or_unsafe_reads_have_typed_errors() {
    let f = Fixture::start().await;
    for path in [
        "/vaults?limit=0",
        "/vaults?limit=101",
        "/vaults?limit=1&limit=2",
        "/vaults?root=/private",
        "/capabilities?scope=write",
        "/vaults?cursor=%FF",
        "/vaults?cursor=%ZZ",
    ] {
        error(
            f.get(path).send().await.unwrap(),
            StatusCode::BAD_REQUEST,
            ContentErrorCode::InvalidRequest,
        )
        .await;
    }
    for path in ["../outside.md", ".lapis/private.md"] {
        error(
            f.get(&f.note_path())
                .query(&[("path", path)])
                .send()
                .await
                .unwrap(),
            StatusCode::BAD_REQUEST,
            ContentErrorCode::InvalidRequest,
        )
        .await;
    }
    error(
        f.get(&f.note_path())
            .query(&[("path", "missing.md")])
            .send()
            .await
            .unwrap(),
        StatusCode::NOT_FOUND,
        ContentErrorCode::NotFound,
    )
    .await;
    fs::write(
        f.root.path().join("large.md"),
        vec![b'x'; MAX_CONTENT_TEXT_BYTES + 1],
    )
    .unwrap();
    error(
        f.get(&f.note_path())
            .query(&[("path", "large.md")])
            .send()
            .await
            .unwrap(),
        StatusCode::PAYLOAD_TOO_LARGE,
        ContentErrorCode::TooLarge,
    )
    .await;
}

#[tokio::test]
async fn metadata_body_and_correlation_limits_are_enforced_and_vault_close_is_rechecked() {
    let f = Fixture::start().await;
    let prefix = "/vaults?cursor=";
    let one_over = format!(
        "{prefix}{}",
        "x".repeat(16 * 1024 + 1 - "/api/content/v1".len() - prefix.len())
    );
    error(
        f.get(&one_over).send().await.unwrap(),
        StatusCode::PAYLOAD_TOO_LARGE,
        ContentErrorCode::TooLarge,
    )
    .await;
    error(
        f.get("/vaults")
            .query(&[("cursor", "x".repeat(17 * 1024))])
            .send()
            .await
            .unwrap(),
        StatusCode::PAYLOAD_TOO_LARGE,
        ContentErrorCode::TooLarge,
    )
    .await;
    error(
        f.get("/vaults")
            .header("x-extra", "x".repeat(9 * 1024))
            .send()
            .await
            .unwrap(),
        StatusCode::PAYLOAD_TOO_LARGE,
        ContentErrorCode::TooLarge,
    )
    .await;
    error(
        f.get("/capabilities")
            .body("unexpected")
            .send()
            .await
            .unwrap(),
        StatusCode::BAD_REQUEST,
        ContentErrorCode::InvalidRequest,
    )
    .await;
    for id in ["not-a-uuid".to_string(), Uuid::nil().to_string()] {
        let response = f
            .get("/capabilities")
            .header(REQUEST_ID_HEADER, id)
            .send()
            .await
            .unwrap();
        assert!(
            !Uuid::parse_str(response.headers()[REQUEST_ID_HEADER].to_str().unwrap())
                .unwrap()
                .is_nil()
        );
        error(
            response,
            StatusCode::BAD_REQUEST,
            ContentErrorCode::InvalidRequest,
        )
        .await;
    }
    fs::write(f.root.path().join("invalid.md"), [0xff]).unwrap();
    error(
        f.get(&f.note_path())
            .query(&[("path", "invalid.md")])
            .send()
            .await
            .unwrap(),
        StatusCode::BAD_REQUEST,
        ContentErrorCode::InvalidRequest,
    )
    .await;
    f.runtime.close_vault(f.vault).unwrap();
    error(
        f.get(&f.note_path())
            .query(&[("path", "a note.md")])
            .send()
            .await
            .unwrap(),
        StatusCode::SERVICE_UNAVAILABLE,
        ContentErrorCode::Unavailable,
    )
    .await;
}
