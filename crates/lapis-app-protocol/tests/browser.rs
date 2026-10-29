use lapis_app_protocol::browser::{LoginRequest, SessionInfo};
use serde_json::json;

#[test]
fn browser_credentials_cannot_carry_a_native_path_or_a_claimed_identity() {
    assert!(
        serde_json::from_value::<LoginRequest>(json!({"username":"owner","password":"disposable"}))
            .is_ok()
    );
    for field in ["root", "session_id", "role", "user_id"] {
        let mut value = json!({"username":"owner","password":"disposable"});
        value[field] = json!("untrusted");
        assert!(
            serde_json::from_value::<LoginRequest>(value).is_err(),
            "{field}"
        );
    }
    let info: SessionInfo = serde_json::from_value(
        json!({"username":"owner","csrf_token":"csrf","expires_at":1700000000000_i64}),
    )
    .unwrap();
    assert_eq!(info.expires_at, 1700000000000);
}
