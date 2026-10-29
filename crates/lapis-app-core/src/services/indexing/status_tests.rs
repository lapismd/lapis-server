use super::{IndexingStatus, TrackedTaskStatus};
use lapis_app_protocol::indexing::IndexingPhase;

#[test]
fn native_tracking_and_shared_wire_progress_retain_separate_contracts() {
    let mut native = IndexingStatus {
        tracked: Some(TrackedTaskStatus {
            date: "2026-10-26".into(),
            materialized: 1,
            ..Default::default()
        }),
        ..Default::default()
    };
    native.passes = 7;
    native.phase = IndexingPhase::Tasks;
    assert_eq!(native.tracked.as_ref().unwrap().materialized, 1);
    let wire = serde_json::to_value(native.progress).unwrap();
    assert_eq!(wire["passes"], 7);
    assert_eq!(wire["phase"], "tasks");
    assert!(wire.get("tracked").is_none());
    assert!(wire.get("progress").is_none());
    let decoded: lapis_app_protocol::indexing::IndexingStatus =
        serde_json::from_value(wire).unwrap();
    assert_eq!(decoded.passes, 7);
}
