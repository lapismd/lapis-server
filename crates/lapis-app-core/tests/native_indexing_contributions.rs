use lapis_app_core::{
    embedding::host::NativeEmbeddingHost,
    services::{DatabaseStorage, Runtime},
};
use lapis_app_protocol::indexing::*;
use uuid::Uuid;

#[tokio::test]
async fn configured_client_projections_are_not_mistaken_for_compiled_parity() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let runtime = Runtime::with_database(
        DatabaseStorage::new(profile.path(), NativeEmbeddingHost::default()).unwrap(),
    );
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let client = vault.attach().unwrap();
    for (owner, id, version) in [
        ("docs", "docs/note", 1.0),
        (
            "tasks",
            "tasks/task",
            f64::from(lapis_task_services::TASK_PROJECTION_VERSION),
        ),
        (
            "tasks",
            "tasks/occurrence",
            f64::from(lapis_task_services::TASK_OCCURRENCE_PROJECTION_VERSION),
        ),
    ] {
        let response = client
            .control_indexing(IndexingCommand::RegisterProjection {
                contribution: ProjectionContribution {
                    projection_id: id.into(),
                    owner_plugin_id: owner.into(),
                    schema_version: version,
                    configuration_hash: "unqualified-custom-configuration".into(),
                    path_prefix: None,
                },
            })
            .await
            .unwrap();
        assert!(response.status.is_none(), "{id} selected a native owner");
        assert!(
            matches!(
                response.contribution.unwrap().placement,
                ContributionPlacement::ClientDependent
            ),
            "{id} claimed parity with an unqualified configuration"
        );
    }
    drop(client);
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn both_retained_markdown_extensions_select_the_native_processor() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    std::fs::write(root.path().join("Note.markdown"), "# Markdown\n").unwrap();
    let runtime = Runtime::with_database(
        DatabaseStorage::new(profile.path(), NativeEmbeddingHost::default()).unwrap(),
    );
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let client = vault.attach().unwrap();
    for extension in ["markdown", "md"] {
        let response = client
            .control_indexing(IndexingCommand::RegisterProcessor {
                owner_plugin_id: "markdown".into(),
                extension: extension.into(),
            })
            .await
            .unwrap();
        assert!(response.status.is_some());
        assert!(matches!(
            response.contribution.unwrap().placement,
            ContributionPlacement::Native
        ));
    }
    let status = client
        .control_indexing(IndexingCommand::Refresh { rebuild: false })
        .await
        .unwrap()
        .status
        .unwrap();
    assert_eq!(status.total, 1);
    assert!(status.error.is_none(), "{:?}", status.error);
    drop(client);
    tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .unwrap()
        .unwrap();
}
