use super::support::Fixture;
use lapis_app_protocol::{
    ErrorCode, Outcome, VaultCommand, VaultValue,
    editing_intents::{EditingIntentCommand, EditingIntentValue},
};
use std::fs;
use uuid::Uuid;

fn intent(command: EditingIntentCommand) -> VaultCommand {
    VaultCommand::EditingIntents { command }
}

#[tokio::test]
async fn editor_recovery_uses_same_client_authority_and_captured_state_over_real_http() {
    let f = Fixture::start().await;
    fs::write(f.roots[0].path().join("note.md"), "saved bytes").unwrap();
    let owner = f.attach(f.ids[0]).await;
    let peer = f.attach(f.ids[0]).await;
    let foreign = f.attach(f.ids[1]).await;
    let id = Uuid::new_v4();
    assert!(matches!(
        f.execute(
            &owner,
            intent(EditingIntentCommand::Acquire {
                editor_id: id,
                path: "note.md".into(),
            })
        )
        .await
        .outcome,
        Outcome::Ok { .. }
    ));
    let recover = || {
        intent(EditingIntentCommand::Recover {
            editor_id: id,
            generation: 1,
            revision: 1,
        })
    };
    let discard = |generation, revision| {
        intent(EditingIntentCommand::Discard {
            editor_id: id,
            generation,
            revision,
        })
    };
    let bad_origin = f
        .command(&peer, discard(1, 0))
        .header(reqwest::header::ORIGIN, "https://foreign.invalid")
        .send()
        .await
        .unwrap();
    assert_eq!(bad_origin.status(), 403);
    assert!(matches!(f.execute(&foreign, discard(1, 0)).await.outcome,
        Outcome::Error { error } if error.code == ErrorCode::PermissionDenied));
    f.execute(
        &owner,
        intent(EditingIntentCommand::Changed {
            editor_id: id,
            generation: 1,
            revision: 1,
        }),
    )
    .await;
    f.detach(&owner).await;
    assert!(matches!(f.execute(&peer, discard(1, 0)).await.outcome,
        Outcome::Error { error } if error.code == ErrorCode::Conflict));
    assert!(
        matches!(f.execute(&peer, VaultCommand::RecoverEditor { editor_id: id }).await.outcome,
        Outcome::Error { error } if error.code == ErrorCode::InvalidRequest)
    );
    let recovered = f.execute(&peer, recover()).await;
    assert!(matches!(recovered.outcome, Outcome::Ok {
        value: VaultValue::EditingIntents { value: EditingIntentValue::State { state } }
    } if state.generation == 2 && state.revision == 1 && state.active && state.dirty && !state.detached));
    assert!(matches!(f.execute(&peer, discard(1, 1)).await.outcome,
        Outcome::Error { error } if error.code == ErrorCode::Conflict));
    assert_eq!(f.host.shutdown().unwrap_err().code, ErrorCode::Busy);
    // The caller abandons the response body; inspection proves the terminal effect.
    let response = f.command(&peer, discard(2, 1)).send().await.unwrap();
    assert!(response.status().is_success());
    drop(response);
    assert!(matches!(
        f.execute(
            &peer,
            intent(EditingIntentCommand::Inspect { editor_id: id })
        )
        .await
        .outcome,
        Outcome::Ok {
            value: VaultValue::EditingIntents {
                value: EditingIntentValue::Retired {}
            }
        }
    ));
    assert!(matches!(
        f.execute(&peer, discard(2, 1)).await.outcome,
        Outcome::Ok {
            value: VaultValue::EditingIntents {
                value: EditingIntentValue::Retired {}
            }
        }
    ));
    assert_eq!(
        fs::read_to_string(f.roots[0].path().join("note.md")).unwrap(),
        "saved bytes"
    );
    f.detach(&peer).await;
    f.detach(&foreign).await;
    f.host.shutdown().unwrap();
}
