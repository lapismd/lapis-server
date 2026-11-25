use lapis_app_core::credentials::{CredentialError, SecretBytes, SecretName};

#[test]
fn names_are_bounded_and_preserve_opaque_identifiers() {
    let name = SecretName::new("forge:github.com:account").unwrap();
    assert_eq!(name.as_str(), "forge:github.com:account");
    assert!(matches!(
        SecretName::new(""),
        Err(CredentialError::InvalidInput)
    ));
    assert!(SecretName::new("line\nbreak").is_err());
    assert!(SecretName::new("a".repeat(513)).is_err());
}

#[test]
fn values_are_bounded_opaque_bytes() {
    let bytes = SecretBytes::new(vec![0, 255]).unwrap();
    assert_eq!(bytes.expose_secret(), &[0, 255]);
    assert!(SecretBytes::new(vec![0; 32 * 1024]).is_ok());
    assert!(matches!(
        SecretBytes::new(vec![0; 32 * 1024 + 1]),
        Err(CredentialError::InvalidInput)
    ));
}
