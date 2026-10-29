use super::{BrowserFixture, PROTOCOL_MAJOR, VERSION_HEADER};
use lapis_app_protocol::{Outcome, VaultCommand, VaultValue};

#[tokio::test]
async fn cookie_module_requests_keep_session_origin_and_version_fences() {
    use sha2::{Digest, Sha256};
    let mut f = BrowserFixture::start(true, None).await;
    let bytes = b"export const marker = 1;";
    let hash = format!("{:x}", Sha256::digest(bytes));
    let fingerprint = format!(
        "{:x}",
        Sha256::digest(format!("main.mjs:{}:{hash}", bytes.len()))
    );
    std::fs::create_dir_all(f.vault.join(".lapis/plugins/portable")).unwrap();
    std::fs::write(f.vault.join(".lapis/plugins/portable/main.mjs"), bytes).unwrap();
    std::fs::write(f.vault.join(".lapis/installed-plugins.json"), serde_json::json!({"schemaVersion":1,"plugins":{"portable":{"pluginId":"portable","installedVersion":"1.0.0","files":[{"path":"main.mjs","size":bytes.len(),"sha256":hash}]}}}).to_string()).unwrap();
    let (a, info, _) = f.login().await;
    let (b, _, _) = f.login().await;
    let attached = f.attach(&a, &info).await;
    let url = format!(
        "{}/__lapis/plugins/{}/portable/1.0.0/{fingerprint}/main.mjs",
        f.base, attached.client_id
    );
    let response = f
        .client
        .get(&url)
        .header("cookie", &a)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.bytes().await.unwrap().as_ref(), bytes);
    for (request, expected) in [
        (f.client.get(&url), 401),
        (f.client.get(&url).header("cookie", &b), 403),
        (
            f.client
                .get(&url)
                .header("cookie", &a)
                .header("origin", "https://foreign.invalid"),
            403,
        ),
        (
            f.client
                .get(&url)
                .header("cookie", &a)
                .header(VERSION_HEADER, "2"),
            409,
        ),
    ] {
        assert_eq!(request.send().await.unwrap().status(), expected);
    }
    let response = f
        .authorize(
            f.client.post(format!("{}/api/v3/auth/logout", f.base)),
            &a,
            &info,
            true,
        )
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        f.client
            .get(&url)
            .header("cookie", &a)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    f.finish().await;
}

#[tokio::test]
async fn sessions_and_operator_tokens_cannot_claim_each_others_clients() {
    let token = super::super::support::TOKEN;
    let mut f = BrowserFixture::start(true, Some(token)).await;
    let (a, ainfo, _) = f.login().await;
    let (b, binfo, _) = f.login().await;
    let attached = f.attach(&a, &ainfo).await;
    assert_eq!(
        f.command(&b, &binfo, &attached, VaultCommand::Status {})
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    let operator = f
        .client
        .post(format!(
            "{}/api/v3/clients/{}/heartbeat",
            f.base, attached.client_id
        ))
        .bearer_auth(token)
        .header(VERSION_HEADER, PROTOCOL_MAJOR)
        .send()
        .await
        .unwrap();
    assert_eq!(operator.status(), 403);
    let other = f.attach(&b, &binfo).await;
    let logout = f
        .authorize(
            f.client.post(format!("{}/api/v3/auth/logout", f.base)),
            &a,
            &ainfo,
            true,
        )
        .send()
        .await
        .unwrap();
    assert_eq!(logout.status(), 200);
    assert_eq!(
        f.command(&a, &ainfo, &attached, VaultCommand::Status {})
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        f.command(&b, &binfo, &other, VaultCommand::Status {})
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    f.finish().await;
}

#[tokio::test]
async fn logout_retains_dirty_claims_and_cookie_media_requests_need_no_custom_header() {
    let mut f = BrowserFixture::start(true, None).await;
    let (cookie, info, _) = f.login().await;
    let attached = f.attach(&cookie, &info).await;
    std::fs::write(f.vault.join("image.svg"), "<svg>image</svg>").unwrap();
    let resource = format!(
        "{}/api/v3/clients/{}/resources/image.svg",
        f.base, attached.client_id
    );
    assert_eq!(
        f.client
            .get(&resource)
            .header("cookie", &cookie)
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        f.client
            .get(&resource)
            .header("cookie", &cookie)
            .header(VERSION_HEADER, 2)
            .send()
            .await
            .unwrap()
            .status(),
        409
    );
    let reply: lapis_app_protocol::Reply = f
        .command(
            &cookie,
            &info,
            &attached,
            VaultCommand::OpenEditor {
                path: "draft.md".into(),
            },
        )
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let editor_id = match reply.outcome {
        Outcome::Ok {
            value: VaultValue::Editor { editor_id },
        } => editor_id,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        f.command(
            &cookie,
            &info,
            &attached,
            VaultCommand::MarkDirty {
                editor_id,
                revision: 1
            }
        )
        .send()
        .await
        .unwrap()
        .status(),
        200
    );
    assert_eq!(
        f.authorize(
            f.client.post(format!("{}/api/v3/auth/logout", f.base)),
            &cookie,
            &info,
            true
        )
        .send()
        .await
        .unwrap()
        .status(),
        200
    );
    assert_eq!(
        f.client
            .get(&resource)
            .header("cookie", &cookie)
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    let (fresh, finfo, _) = f.login().await;
    let next = f.attach(&fresh, &finfo).await;
    let claims: lapis_app_protocol::Reply = f
        .command(&fresh, &finfo, &next, VaultCommand::EditorClaims {})
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        matches!(claims.outcome,Outcome::Ok { value:VaultValue::EditorClaims { claims } } if claims.len()==1 && claims[0].detached)
    );
    for command in [
        VaultCommand::RecoverEditor { editor_id },
        VaultCommand::DiscardEditor { editor_id },
    ] {
        assert_eq!(
            f.command(&fresh, &finfo, &next, command)
                .send()
                .await
                .unwrap()
                .status(),
            200
        );
    }
    f.finish().await;
}
