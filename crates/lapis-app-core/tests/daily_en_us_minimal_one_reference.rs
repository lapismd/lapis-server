mod support;
use serde_json::Value;
fn fixture() -> Value {
    serde_json::from_str(include_str!(
        "fixtures/daily-provider-en-us-minimal-one.json"
    ))
    .unwrap()
}
#[test]
fn daily_en_us_minimal_one_filenames_match_all_retained_outputs_and_errors() {
    support::daily_reference::filenames(&fixture());
}
#[test]
fn daily_en_us_minimal_one_provider_preserves_all_retained_bytes_paths_and_errors() {
    support::daily_reference::providers(&fixture());
}
#[test]
fn daily_en_us_minimal_one_registry_preserves_all_retained_steps() {
    support::daily_reference::registry(&fixture());
}
