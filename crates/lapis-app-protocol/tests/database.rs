use lapis_app_protocol::database::projections::{ProjectionDefinition, ProjectionSource};
use lapis_app_protocol::database::{MetaEntry, MetaPage, MetaTransaction};
use serde_json::json;

#[test]
fn metadata_dtos_retain_optional_and_null_values_and_camelcase_pagination() {
    let tx: MetaTransaction = serde_json::from_value(json!({"mutations":[
        {"key":"null","value":null},{"key":"delete"}
    ]}))
    .unwrap();
    assert_eq!(tx.mutations[0].value, Some(json!(null)));
    assert_eq!(tx.mutations[1].value, None);
    assert_eq!(
        serde_json::to_value(&tx).unwrap()["mutations"][0]["value"],
        json!(null)
    );
    assert!(
        serde_json::to_value(&tx).unwrap()["mutations"][1]
            .get("value")
            .is_none()
    );
    for field in ["sql", "path", "vaultId"] {
        let mut value = json!({"mutations":[]});
        value[field] = json!("untrusted");
        assert!(serde_json::from_value::<MetaTransaction>(value).is_err());
    }
    assert_eq!(
        serde_json::to_value(MetaPage {
            entries: vec![MetaEntry {
                key: "key".into(),
                value: json!(null)
            }],
            next_cursor: Some("last".into())
        })
        .unwrap(),
        json!({"entries":[{"key":"key","value":null}],"nextCursor":"last"})
    );
}

#[test]
fn projection_contracts_retain_typed_namespaces_ordered_fields_and_reject_sql() {
    let value = json!({"projectionId":"books/book","ownerPluginId":"books","schemaVersion":2,"configHash":"v2","visibility":"public","fields":{"z":{"type":"string","indexed":true},"a":{"type":"datetime","sortable":true}},"active":true,"updatedAt":10});
    let definition: ProjectionDefinition = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(
        definition
            .fields
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["z", "a"]
    );
    assert_eq!(
        serde_json::to_value(definition).unwrap()["visibility"],
        json!("public")
    );
    let mut bad = value.clone();
    bad["visibility"] = json!("shared");
    assert!(serde_json::from_value::<ProjectionDefinition>(bad).is_err());
    let mut bad = value;
    bad["fields"]["a"]["type"] = json!("raw-sql");
    assert!(serde_json::from_value::<ProjectionDefinition>(bad).is_err());
    let source:ProjectionSource=serde_json::from_value(json!({"projectionId":"books/book","sourcePath":"a.md","sourceHash":"h","rows":[{"id":"a","kind":"book","data":{"null":null,"object":{"x":1}}}]})).unwrap();
    assert!(source.edges.is_empty());
    assert!(source.writer_plugin_id.is_none());
    let mut bad = serde_json::to_value(source).unwrap();
    bad["sql"] = json!("DELETE FROM files");
    assert!(serde_json::from_value::<ProjectionSource>(bad).is_err());
}
