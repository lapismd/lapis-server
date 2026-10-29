use lapis_app_core::embedding::OnnxEncoder;
use lapis_app_protocol::ErrorCode;

#[test]
fn missing_native_runtime_is_explicit_and_never_replaced_with_another_encoder() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("not-an-installed-runtime");
    let error = OnnxEncoder::open(&path, &path, &path, &path, true)
        .err()
        .expect("missing runtime must fail");
    assert_eq!(error.code(), ErrorCode::Unsupported);
    let message = error.to_string();
    assert!(!message.contains(root.path().to_str().unwrap()));
}
