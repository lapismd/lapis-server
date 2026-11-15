use lapis_app_protocol::repository_sync::RepositorySyncCommand;
use serde_json::json;
#[test]
fn synchronization_controls_accept_only_admitted_recovery_identities() {
    for value in [
        json!({"kind":"run", "expected_revision":2}),
        json!({"kind":"resume_apply", "expected_revision":2, "request_id":"retained-request"}),
        json!({"kind":"inspect", "expected_revision":2, "candidate":"abc", "path":"Note.md"}),
        json!({"kind":"resolve", "expected_revision":2, "candidate":"abc", "path":"Note.md", "resolution":{"kind":"term", "id":1}}),
    ] {
        assert!(serde_json::from_value::<RepositorySyncCommand>(value.clone()).is_ok());
        for field in [
            "root",
            "metadata",
            "window",
            "vault",
            "generation",
            "credentials",
            "token",
            "url",
        ] {
            let mut injected = value.clone();
            injected[field] = json!("outside");
            assert!(serde_json::from_value::<RepositorySyncCommand>(injected).is_err());
        }
    }
    assert!(serde_json::from_value::<RepositorySyncCommand>(json!({"kind":"resolve", "expected_revision":2, "candidate":"abc", "path":"Note.md", "resolution":{"kind":"text", "text":"resolved", "executable":false, "token":"outside"}})).is_err());
}
