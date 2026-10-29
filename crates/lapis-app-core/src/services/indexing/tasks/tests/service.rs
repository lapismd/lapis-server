use super::support::*;
use lapis_app_protocol::database::call::DatabaseMethod;
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn private_selection_wakes_one_owner_and_preserves_core_metadata_and_public_rows() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let raw = document(
        "list_root",
        "task-list",
        "## Work 📝\n\n- [Child](../tasks/Child.md)\n\n[[Reference]]",
    );
    write(&root, "lists/Root.md", &raw);
    write(
        &root,
        "tasks/Child.md",
        &document("task_child", "task", "- [ ] Check"),
    );
    write(&root, "Reference.md", "# Reference\n");
    write(
        &root,
        "Navigation.md",
        &document(
            "navigation",
            "task-list-navigation",
            "- [Root](lists/Root.md)",
        ),
    );
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    vault.start_markdown_indexing().await.unwrap();
    until(async || vault.indexing_status().unwrap().passes > 0).await;
    assert!(query(&vault).await.is_err());
    vault.set_task_document_indexing(true).await.unwrap();
    until(async || {
        query(&vault).await.ok().is_some_and(|result| {
            result["rows"]
                .as_array()
                .is_some_and(|rows| rows.len() == 3)
        })
    })
    .await;
    let task = row(&vault, "lists/Root.md").await;
    assert_eq!(task["documentId"], "list_root");
    assert_eq!(
        task["structure"]["entries"][0]["targetDocumentId"],
        "task_child"
    );
    let children = call(
        &vault,
        DatabaseMethod::ListChildLinks,
        vec![json!({"sourcePath":"lists/Root.md"})],
    )
    .await
    .unwrap();
    assert_eq!(children.as_array().unwrap().len(), 1);
    assert_eq!(children[0]["kind"], "task-entry");
    assert_eq!(children[0]["heading"], "Work 📝");
    let indexed = call(
        &vault,
        DatabaseMethod::GetIndexedFile,
        vec![json!("lists/Root.md")],
    )
    .await
    .unwrap();
    assert_eq!(
        indexed["metadata"]["metadata"]["headings"][1]["heading"],
        "Work 📝"
    );
    assert!(
        indexed["links"]
            .as_array()
            .unwrap()
            .iter()
            .all(|link| link["kind"] == "reference")
    );
    assert_ne!(indexed["file"]["hash"], "list_root");
    assert!(indexed["file"]["mtime"].as_f64().unwrap() > 1.0);
    assert_eq!(indexed["file"]["size"].as_f64().unwrap(), raw.len() as f64);
    assert_eq!(vault.status().unwrap().clients, 0);
    close(runtime).await;
}

#[tokio::test]
async fn target_parent_and_cycle_edits_refresh_unchanged_source_documents() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let parent = document("parent", "task-list", "- [Child](Child.md)");
    write(&root, "Parent.md", &parent);
    write(&root, "Child.md", &document("child", "task", ""));
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    vault.set_task_document_indexing(true).await.unwrap();
    until(async || {
        row(&vault, "Parent.md").await["structure"]["entries"]
            .as_array()
            .is_some_and(|v| v.len() == 1)
    })
    .await;
    write(&root, "Child.md", &document("child", "task-list", ""));
    until(async || {
        row(&vault, "Parent.md").await["structure"]["entries"][0]["kind"] == "list-entry"
    })
    .await;
    write(
        &root,
        "Other.md",
        &document("other", "task", "- [Child](Child.md)"),
    );
    until(async || {
        row(&vault, "Parent.md").await["structure"]["diagnostics"][0]["code"]
            == "multiple-structural-parents"
    })
    .await;
    assert_eq!(
        row(&vault, "Other.md").await["structure"]["entries"],
        json!([])
    );
    std::fs::remove_file(root.path().join("Other.md")).unwrap();
    until(async || {
        row(&vault, "Parent.md").await["structure"]["entries"]
            .as_array()
            .is_some_and(|v| v.len() == 1)
    })
    .await;
    write(
        &root,
        "Child.md",
        &document("child", "task-list", "- [Parent](Parent.md)"),
    );
    until(async || {
        row(&vault, "Parent.md").await["structure"]["diagnostics"][0]["code"] == "structural-cycle"
    })
    .await;
    assert_eq!(
        std::fs::read_to_string(root.path().join("Parent.md")).unwrap(),
        parent
    );
    close(runtime).await;
}

#[tokio::test]
async fn admitted_basename_targets_resolve_but_ordinary_hidden_and_symlink_targets_do_not() {
    let profile = tempfile::tempdir().unwrap();
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    write(
        &root,
        "Parent.md",
        &document(
            "parent",
            "task-list",
            "- [Child](Child)\n- [Note](Note.md)\n- [Hidden](.lapis/Hidden.md)\n- [Escape](Escape.md)",
        ),
    );
    write(&root, "nested/Child.md", &document("child", "task", ""));
    write(&root, "Note.md", "# Ordinary\n");
    write(&root, ".lapis/Hidden.md", &document("hidden", "task", ""));
    write(&outside, "Secret.md", &document("secret", "task", ""));
    std::os::unix::fs::symlink(
        outside.path().join("Secret.md"),
        root.path().join("Escape.md"),
    )
    .unwrap();
    let runtime = runtime(&profile);
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    vault.set_task_document_indexing(true).await.unwrap();
    until(async || {
        row(&vault, "Parent.md").await["structure"]["diagnostics"]
            .as_array()
            .is_some_and(|v| v.len() == 3)
    })
    .await;
    let task = row(&vault, "Parent.md").await;
    assert_eq!(
        task["structure"]["entries"][0]["targetDocumentPath"],
        "nested/Child.md"
    );
    assert_eq!(
        task["structure"]["diagnostics"][0]["code"],
        "unsupported-structural-target"
    );
    assert_eq!(
        task["structure"]["diagnostics"][1]["code"],
        "unresolved-structural-link"
    );
    assert_eq!(
        task["structure"]["diagnostics"][2]["code"],
        "unresolved-structural-link"
    );
    assert!(row(&vault, ".lapis/Hidden.md").await.is_null());
    close(runtime).await;
}
