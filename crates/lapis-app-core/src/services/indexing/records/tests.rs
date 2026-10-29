#[test]
fn retained_application_normalized_record_fixtures() {
    let fixtures: Vec<serde_json::Value> = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/native-indexed-metadata.json"
    )))
    .unwrap();
    for mut fixture in fixtures {
        let files: Vec<String> = serde_json::from_value(fixture["files"].clone()).unwrap();
        let stat = crate::vault::FileStat {
            r#type: "file",
            ctime: 0.0,
            mtime: fixture["mtime"].as_f64().unwrap(),
            size: fixture["size"].as_u64().unwrap(),
        };
        let record = super::record(
            fixture["path"].as_str().unwrap(),
            &stat,
            fixture["text"].as_str().unwrap(),
            &files,
            fixture["declared"].as_object().unwrap(),
        );
        fixture["expected"]["metadata"]["parserVersion"] =
            serde_json::json!(lapis_plugin_services::markdown::PARSER_VERSION);
        let expected: lapis_app_protocol::database::indexed::records::CoreIndexedFile =
            serde_json::from_value(fixture["expected"].clone()).unwrap();
        assert_eq!(
            serde_json::to_value(record).unwrap(),
            serde_json::to_value(expected).unwrap(),
            "{}",
            fixture["name"]
        );
    }
}
