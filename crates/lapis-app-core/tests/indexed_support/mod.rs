use lapis_app_protocol::database::indexed::records::CoreIndexedFile;
use serde_json::json;

pub fn record(path: &str, score: i64) -> CoreIndexedFile {
    serde_json::from_value(json!({
        "file":{"path":path,"normalizedPath":path,"extension":".MD","mtime":score,"size":12,"hash":format!("hash-{score}"),"indexed":true},
        "metadata":{"path":path,"hash":format!("hash-{score}"),"parserVersion":"parser-1","metadata":{"heading":"Native metadata"}},
        "links":[{"sourcePath":path,"targetText":"Target","resolvedTargetPath":"target.md","type":"link","count":2,"kind":"reference"}],
        "tags":[{"path":path,"tag":"#Work/Notes","parts":["Work","Notes"],"hierarchy":["Work","Work/Notes"]}],
        "properties":[{"path":path,"name":"Score","inferredType":"number","value":score},{"path":path,"name":"Team","inferredType":"object","value":{"members":[{"name":"Ada","active":true}]}},{"path":path,"name":"Due","inferredType":"date","value":"2026-10-24"}]
    })).unwrap()
}
