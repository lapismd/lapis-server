#[test]
fn bundled_cli_help_matches_the_pinned_tasks_provider() {
    let bundled: serde_json::Value =
        serde_json::from_str(include_str!("../../lapis-cli/help/tasks-operations.json")).unwrap();
    assert_eq!(
        bundled,
        serde_json::to_value(lapis_task_operations::descriptors()).unwrap()
    );
}
