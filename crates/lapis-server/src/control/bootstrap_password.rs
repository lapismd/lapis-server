use super::{password_source_message, select_owner_password};

#[test]
fn environment_password_overrides_the_file_and_is_not_logged() {
    let environment = "env-fixture";
    let file = "file-fixture\n";
    let selected = select_owner_password(Some(environment), Some(file)).unwrap();
    assert_eq!(selected, environment);
    let message = password_source_message(Some(environment));
    assert_eq!(message, "owner password source: environment");
    assert!(!message.contains(environment));
    assert!(!message.contains("file-fixture"));

    let from_file = select_owner_password(None, Some(file)).unwrap();
    assert_eq!(from_file, "file-fixture");
    assert_eq!(
        password_source_message(None),
        "owner password source: file"
    );
    assert!(!password_source_message(Some("")).contains("file-fixture"));
}
