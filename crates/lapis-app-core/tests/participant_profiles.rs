#[path = "support/participant_profiles/mod.rs"]
mod support;
use std::{fs, sync::atomic::Ordering};
use support::{Fixture, input, runtime, shutdown};
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn declared_catalog_bounds_and_opaque_runtime_associations_are_independent_of_message_bounds()
{
    let f = Fixture::new();
    let (runtime, policy) = runtime(&f);
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let template = policy.profiles.lock().unwrap()[0].clone();
    let profiles: Vec<_> = (0..16)
        .map(|index| {
            let mut row = template.clone();
            let id = format!("role-{index}");
            let token: String = id.as_bytes().iter().map(|b| format!("{b:02x}")).collect();
            row.1.recipient.id = id.clone();
            row.1.recipient.token = format!("jj-store-agent:{token};");
            row.1.target.id = id;
            row
        })
        .collect();
    *policy.profiles.lock().unwrap() = profiles.clone();
    assert_eq!(client.participant_profiles().unwrap().len(), 16);
    for index in 0..4 {
        let mut changed = profiles.clone();
        match index {
            0 => changed.push(template.clone()),
            1 => changed[1] = changed[0].clone(),
            2 => changed[0].0 = "uninstalled-runtime".into(),
            _ => changed[0].1.recipient.token = "jj-store-agent:00;".into(),
        }
        *policy.profiles.lock().unwrap() = changed;
        assert!(client.participant_profiles().is_err(), "{index}");
    }
    assert_eq!(f.policy.calls.load(Ordering::SeqCst), 0);
    assert!(fs::read_dir(&f.root).unwrap().next().is_none());
    drop(client);
    shutdown(runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selected_profile_snapshots_refuse_stale_declarations_before_claims_but_raw_inputs_remain_compatible()
 {
    let f = Fixture::new();
    let (runtime, policy) = runtime(&f);
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let profiles = client.participant_profiles().unwrap();
    assert_eq!(profiles.len(), 2);
    assert!(profiles.iter().all(|p| p.options.effort.is_none()));
    let captured = input(&f, profiles.clone());
    policy.profiles.lock().unwrap()[0].1.recipient.label = "Renamed after capture".into();
    let before = f.policy.calls.load(Ordering::SeqCst);
    assert!(client.submit_participant_message(captured.clone()).is_err());
    assert!(
        client
            .participant_message(captured.acceptance_id)
            .unwrap()
            .is_none()
    );
    for turn in &captured.turns {
        assert!(client.turn(turn.request_id).unwrap().is_none());
    }
    assert_eq!(f.policy.calls.load(Ordering::SeqCst), before);
    assert!(fs::read_dir(&f.root).unwrap().next().is_none());
    let mut raw = captured.clone();
    raw.profiles = None;
    client.submit_participant_message(raw.clone()).unwrap();
    f.finished(&runtime, raw.turns[1].request_id).await;
    let accepted = client
        .participant_message(raw.acceptance_id)
        .unwrap()
        .unwrap();
    assert_eq!(accepted.input, raw);
    drop(client);
    shutdown(runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn complete_profile_captures_survive_later_configuration_changes_and_exact_retries() {
    let f = Fixture::new();
    let (runtime, policy) = runtime(&f);
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let captured = input(&f, client.participant_profiles().unwrap());
    client.submit_participant_message(captured.clone()).unwrap();
    f.finished(&runtime, captured.turns[1].request_id).await;
    policy.profiles.lock().unwrap().clear();
    let retained = client.submit_participant_message(captured.clone()).unwrap();
    assert_eq!(retained.input, captured);
    assert_eq!(
        client
            .participant_for_turn(captured.turns[0].request_id)
            .unwrap()
            .unwrap(),
        retained
    );
    let before = f.policy.calls.load(Ordering::SeqCst);
    let mut changed = captured.clone();
    changed.acceptance_id = Uuid::new_v4();
    for turn in &mut changed.turns {
        turn.request_id = Uuid::new_v4();
    }
    assert!(client.submit_participant_message(changed).is_err());
    assert_eq!(f.policy.calls.load(Ordering::SeqCst), before);
    drop(client);
    shutdown(runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selected_profile_native_options_and_recipient_associations_cannot_be_rewritten() {
    let f = Fixture::new();
    let (runtime, _policy) = runtime(&f);
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    for index in 0..4 {
        let mut captured = input(&f, client.participant_profiles().unwrap());
        match index {
            0 => captured.profiles.as_mut().unwrap()[0].options.model = Some("foreign".into()),
            1 => captured.profiles.as_mut().unwrap()[0].target_id = "foreign".into(),
            2 => captured.profiles.as_mut().unwrap().reverse(),
            _ => {
                captured.profiles.as_mut().unwrap().pop();
            }
        }
        assert!(
            client.submit_participant_message(captured.clone()).is_err(),
            "{index}"
        );
        assert!(
            client
                .participant_message(captured.acceptance_id)
                .unwrap()
                .is_none()
        );
    }
    assert_eq!(f.policy.calls.load(Ordering::SeqCst), 0);
    drop(client);
    shutdown(runtime).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exact_request_lookup_scans_all_receipts_and_refuses_ambiguity_or_corruption_without_repair()
 {
    let f = Fixture::new();
    let (runtime, _policy) = runtime(&f);
    let client = runtime.get(f.vault).unwrap().attach().unwrap();
    let captured = input(&f, client.participant_profiles().unwrap());
    client.submit_participant_message(captured.clone()).unwrap();
    f.finished(&runtime, captured.turns[1].request_id).await;
    let retained = client
        .participant_message(captured.acceptance_id)
        .unwrap()
        .unwrap();
    let root = f.profile.join("participants").join(f.vault.to_string());
    let path = root.join(format!("{}.json", captured.acceptance_id));
    let original = fs::read(&path).unwrap();
    assert_eq!(
        client
            .participant_for_turn(captured.turns[0].request_id)
            .unwrap()
            .unwrap(),
        retained
    );
    assert_eq!(fs::read(&path).unwrap(), original);
    let mut duplicate = retained.clone();
    duplicate.acceptance_id = Uuid::new_v4();
    duplicate.input.acceptance_id = duplicate.acceptance_id;
    let extra = root.join(format!("{}.json", duplicate.acceptance_id));
    let duplicated = serde_json::to_vec(&duplicate).unwrap();
    fs::write(&extra, &duplicated).unwrap();
    assert!(
        client
            .participant_for_turn(captured.turns[0].request_id)
            .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), original);
    assert_eq!(fs::read(&extra).unwrap(), duplicated);
    fs::write(&extra, b"corrupt").unwrap();
    assert!(
        client
            .participant_for_turn(captured.turns[0].request_id)
            .is_err()
    );
    assert_eq!(fs::read(&extra).unwrap(), b"corrupt");
    assert_eq!(fs::read(&path).unwrap(), original);
    fs::remove_file(extra).unwrap();
    assert!(
        client
            .participant_for_turn(Uuid::new_v4())
            .unwrap()
            .is_none()
    );
    assert!(client.participant_for_turn(Uuid::nil()).is_err());
    drop(client);
    shutdown(runtime).await;
}
