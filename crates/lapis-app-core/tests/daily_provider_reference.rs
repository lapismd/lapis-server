mod support;
use lapis_app_core::services::daily::{
    DailyDocumentSettings, DailyFormatContext, plan_daily_document,
};
use serde_json::Value;
fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/daily-provider.json")).unwrap()
}
#[test]
fn daily_filenames_match_every_retained_luxon_output_and_error() {
    support::daily_reference::filenames(&fixture());
}
#[test]
fn daily_policy_preserves_retained_paths_full_markdown_and_source_errors() {
    support::daily_reference::providers(&fixture());
}
#[test]
fn daily_provider_registry_matches_every_retained_registration_and_disposal_step() {
    support::daily_reference::registry(&fixture());
}
#[test]
fn unqualified_format_context_cannot_create_a_daily_write_intent() {
    for context in [
        DailyFormatContext {
            locale: "en-GB".into(),
            first_day: 7,
            minimal_days: 4,
        },
        DailyFormatContext {
            locale: "en-US".into(),
            first_day: 1,
            minimal_days: 4,
        },
        DailyFormatContext {
            locale: "en-US".into(),
            first_day: 7,
            minimal_days: 2,
        },
    ] {
        let error = plan_daily_document(
            "2026-08-21",
            true,
            &[],
            &DailyDocumentSettings::default(),
            &context,
        )
        .unwrap_err();
        assert_eq!(
            error.message,
            "Native daily formatting context is not qualified."
        );
    }
}
