use lapis_app_protocol::repository_remotes::*;
use serde_json::json;
use ts_rs::{Config, TS};

#[test]
fn remote_requests_have_explicit_targets_and_no_host_authority_fields() {
    let frame = json!({"kind":"fetch", "target":"original", "expectedRevision":3});
    assert!(serde_json::from_value::<RepositoryRemotesCommand>(frame.clone()).is_ok());
    for field in [
        "root",
        "keyFile",
        "credentialDirectory",
        "owner",
        "generation",
        "token",
    ] {
        let mut invalid = frame.clone();
        invalid[field] = json!("outside");
        assert!(
            serde_json::from_value::<RepositoryRemotesCommand>(invalid).is_err(),
            "{field}"
        );
    }
    for field in ["target", "expectedRevision"] {
        let mut invalid = frame.clone();
        invalid.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<RepositoryRemotesCommand>(invalid).is_err());
    }
    let mut invalid = frame;
    invalid["target"] = json!("all");
    assert!(serde_json::from_value::<RepositoryRemotesCommand>(invalid).is_err());
}
#[test]
fn push_requires_one_native_review_and_credentials_use_transient_input_only() {
    let mut push = json!({"kind":"push", "target":"backup", "expectedRevision":3});
    assert!(serde_json::from_value::<RepositoryRemotesCommand>(push.clone()).is_err());
    push["reviewId"] = json!("opaque-native-review");
    assert!(serde_json::from_value::<RepositoryRemotesCommand>(push.clone()).is_ok());
    for field in ["force", "allBookmarks", "commit", "branch", "url"] {
        let mut invalid = push.clone();
        invalid[field] = json!(true);
        assert!(serde_json::from_value::<RepositoryRemotesCommand>(invalid).is_err());
    }
    let credential = json!({"kind":"save_credential", "expectedRevision":3, "input":{
        "label":"Notes", "host":"forge.example", "username":"alex", "token":"synthetic",
        "provider":"https", "replace":null
    }});
    assert!(serde_json::from_value::<RepositoryRemotesCommand>(credential).is_ok());
    let summary = json!({"id":"opaque", "label":"Notes", "host":"forge.example",
        "username":"alex", "method":"token", "available":true, "token":"synthetic"});
    assert!(serde_json::from_value::<RemoteCredentialSummary>(summary).is_err());
}
#[test]
fn generated_commands_match_the_public_presentation_field_names() {
    let declaration = RepositoryRemotesCommand::decl(&Config::default());
    for name in [
        "expectedRevision",
        "reviewId",
        "requestId",
        "save_connection",
        "observe_push",
    ] {
        assert!(
            declaration.contains(name),
            "missing {name} in {declaration}"
        );
    }
    assert!(!declaration.contains("expected_revision"));
    let schema = serde_json::to_string(&schemars::schema_for!(RepositoryRemotesCommand)).unwrap();
    assert!(schema.contains("expectedRevision"));
    assert!(schema.contains("additionalProperties\":false"));
}
