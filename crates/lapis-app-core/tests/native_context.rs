use lapis_app_core::{
    context::{ContextBlock, prepare_attachments, project_agent_turn},
    services::Runtime,
};
use lapis_app_protocol::ErrorCode;
use serde_json::{Value, json};
use std::{
    fs,
    sync::atomic::{AtomicBool, Ordering},
};
use uuid::Uuid;

#[test]
fn native_projection_preserves_authored_text_and_retained_context_format() {
    let blocks: Vec<ContextBlock> = serde_json::from_value(json!([
        {"kind":"memory-recall", "id":"memory", "content":"Remember", "metadata":{}},
        {"kind":"file-context", "id":"file:Notes/a&b.md", "content":"File evidence", "metadata":{"path":"Notes/a&b.md"}},
        {"kind":"transcript-comments", "id":"reply\"<>", "content":"Authored feedback", "metadata":{"submissionId":"reply\"<>"}}
    ])).unwrap();
    let prepared = project_agent_turn("Exact authored café 📝\n", &blocks).unwrap();
    assert_eq!(prepared.text, "Exact authored café 📝\n");
    let context = prepared.context.unwrap();
    assert!(context.contains("each comment's text is explicitly authored user feedback"));
    assert!(context.contains("id=\"reply&quot;&lt;&gt;\""));
    assert!(context.find("File evidence").unwrap() < context.find("Remember").unwrap());
    assert!(!context.contains("Exact authored café"));
}

#[test]
fn native_attachments_use_current_admitted_bytes_after_client_detach() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("note.md"), "captured too early").unwrap();
    let runtime = Runtime::default();
    let vault = runtime.admit(Uuid::new_v4(), root.path()).unwrap();
    let client = vault.attach().unwrap();
    let work = vault.begin_work().unwrap();
    drop(client);
    fs::write(root.path().join("note.md"), "Fresh café 📝").unwrap();
    let result = work
        .attachment_context(&["note.md".into()], || false)
        .unwrap();
    let result: Value = serde_json::to_value(result).unwrap();
    assert_eq!(
        result["blocks"],
        json!([{
            "kind":"file-context", "id":"file:note.md", "content":"File: note.md\n\nFresh café 📝",
            "metadata":{"path":"note.md"}
        }])
    );
    assert_eq!(result["sources"][0]["bytes"], "Fresh café 📝".len());
    assert_eq!(result["sources"][0]["sha256"].as_str().unwrap().len(), 64);
    assert_eq!(
        fs::read_to_string(root.path().join("note.md")).unwrap(),
        "Fresh café 📝"
    );
    assert_eq!(vault.status().unwrap().clients, 0);
}

fn reference() -> Value {
    serde_json::from_str(include_str!("fixtures/ai-context/retained.json")).unwrap()
}

#[test]
fn native_projection_matches_every_complete_retained_source_result() {
    let reference = reference();
    for case in reference["projection"].as_array().unwrap() {
        let input = &case["input"];
        let blocks: Vec<ContextBlock> = serde_json::from_value(input["blocks"].clone()).unwrap();
        let actual = project_agent_turn(input["text"].as_str().unwrap(), &blocks).unwrap();
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            case["expected"],
            "{input}"
        );
    }
}

#[test]
fn native_attachment_preparation_matches_complete_retained_results_and_reads() {
    let reference = reference();
    for case in reference["attachments"].as_array().unwrap() {
        let input = &case["input"];
        let paths: Vec<String> = serde_json::from_value(input["paths"].clone()).unwrap();
        let aborted = AtomicBool::new(input["abort"] == "before");
        let mut reads = Vec::new();
        let actual = prepare_attachments(
            &paths,
            |path| {
                reads.push(path.to_owned());
                if input["abort"] == "after" {
                    aborted.store(true, Ordering::SeqCst);
                }
                Ok(input["files"][path].as_str().map(str::to_owned))
            },
            || aborted.load(Ordering::SeqCst),
        );
        let actual = match actual {
            Ok(value) => json!({"ok":true,"value":value}),
            Err(error) => json!({"ok":false,"error":{
                "name":if error.code() == ErrorCode::Closed {"AbortError"} else {"Error"},
                "message":error.to_string()
            }}),
        };
        assert_eq!(actual, case["expected"], "attachment case paths={paths:?}");
        assert_eq!(
            json!(reads),
            case["reads"],
            "attachment case paths={paths:?}"
        );
    }
}
