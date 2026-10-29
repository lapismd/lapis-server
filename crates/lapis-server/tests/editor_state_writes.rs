pub mod support;

use lapis_app_protocol::{
    ClientDescriptor, ExpectedVersion, Outcome, VaultCommand, editing_intents::EditingIntentCommand,
};
use support::Fixture;
use uuid::Uuid;

fn url(f: &Fixture, client: &ClientDescriptor) -> String {
    format!(
        "{}/api/v3/clients/{}/files/.lapis/app.json",
        f.base, client.client_id
    )
}

async fn intent(f: &Fixture, client: &ClientDescriptor, command: EditingIntentCommand) {
    let reply = f
        .execute(client, VaultCommand::EditingIntents { command })
        .await;
    assert!(matches!(reply.outcome, Outcome::Ok { .. }), "{reply:?}");
}

#[tokio::test]
async fn streamed_conditional_saves_exempt_only_their_own_attached_editor() {
    let f = Fixture::start().await;
    std::fs::create_dir(f.roots[0].path().join(".lapis")).unwrap();
    std::fs::write(f.roots[0].path().join(".lapis/app.json"), "{}").unwrap();
    let owner = f.attach(f.ids[0]).await;
    let foreign = f.attach(f.ids[0]).await;
    let editor_id = Uuid::new_v4();
    intent(
        &f,
        &owner,
        EditingIntentCommand::Acquire {
            editor_id,
            path: ".lapis/app.json".into(),
        },
    )
    .await;

    let original = f
        .authorize(f.client.get(url(&f, &owner)))
        .send()
        .await
        .unwrap();
    let version = original.headers()["etag"].clone();
    let saved = f
        .authorize(f.client.put(url(&f, &owner)))
        .header("if-match", version)
        .body("{\"saved\":true}")
        .send()
        .await
        .unwrap();
    assert_eq!(saved.status(), 204);
    let version = saved.headers()["etag"].clone();
    let blocked = f
        .authorize(f.client.put(url(&f, &foreign)))
        .header("if-match", version.clone())
        .body("{\"unsafe\":true}")
        .send()
        .await
        .unwrap();
    assert_eq!(
        blocked.status(),
        409,
        "A foreign clean active editor remains a fence"
    );

    intent(
        &f,
        &owner,
        EditingIntentCommand::Changed {
            editor_id,
            generation: 1,
            revision: 1,
        },
    )
    .await;
    let saved = f
        .authorize(f.client.put(url(&f, &owner)))
        .header("if-match", version)
        .body("{\"latest\":true}")
        .send()
        .await
        .unwrap();
    assert_eq!(saved.status(), 204);
    let version = saved.headers()["etag"]
        .to_str()
        .unwrap()
        .trim_matches('"')
        .to_owned();
    let blocked = f
        .authorize(f.client.put(url(&f, &foreign)))
        .header("if-match", format!("\"{version}\""))
        .body("{\"unsafe\":true}")
        .send()
        .await
        .unwrap();
    assert_eq!(
        blocked.status(),
        409,
        "A foreign dirty editor remains a fence"
    );
    assert_eq!(
        std::fs::read_to_string(f.roots[0].path().join(".lapis/app.json")).unwrap(),
        "{\"latest\":true}"
    );
    intent(
        &f,
        &owner,
        EditingIntentCommand::Yield {
            editor_id,
            generation: 1,
            revision: 1,
            expected: ExpectedVersion::Version { version },
        },
    )
    .await;
    intent(
        &f,
        &owner,
        EditingIntentCommand::Retire {
            editor_id,
            generation: 1,
        },
    )
    .await;
    f.detach(&owner).await;
    f.detach(&foreign).await;
    f.host.shutdown().unwrap();
}
