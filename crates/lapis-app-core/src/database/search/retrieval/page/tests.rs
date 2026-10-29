use super::*;
use crate::database::search::Value;
use lapis_app_protocol::ErrorCode;
use serde_json::json;

fn large(path: &str) -> SearchDocumentRecord {
    serde_json::from_value(json!({"path":path,"name":"note","extension":"md","checksum":"large","content":"x".repeat(12*1024*1024),"tags":[],"tagParts":[],"tagHierarchy":[]})).unwrap()
}
#[test]
fn path_only_selection_does_not_retain_or_serialize_document_bodies() {
    let mut page = Page::new(3, true).unwrap();
    for path in ["A.md", "B.md", "C.md"] {
        page.push(large(path), 10.).unwrap()
    }
    let Value::Paths(paths) = page
        .finish(&Query::new("note".into()).unwrap(), &Default::default(), 3)
        .unwrap()
    else {
        panic!("path result")
    };
    assert_eq!(paths, ["A.md", "B.md", "C.md"]);
}
#[test]
fn selected_document_buffers_reject_excess_bytes() {
    let mut page = Page::new(3, false).unwrap();
    page.push(large("A.md"), 10.).unwrap();
    page.push(large("B.md"), 10.).unwrap();
    assert_eq!(
        page.push(large("C.md"), 10.).unwrap_err().code(),
        ErrorCode::InvalidRequest
    );
}
