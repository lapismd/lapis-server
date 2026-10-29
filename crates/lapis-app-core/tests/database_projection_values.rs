use lapis_app_core::database::Database;
use serde_json::{Value, json};
use uuid::Uuid;
mod projection_support;
use projection_support::{definition, inspect, source};

#[tokio::test]
async fn native_projection_values_conform_to_the_retained_browser_fixture() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../packages/api/src/lib/__tests__/fixtures/projection-values.json"
    ))
    .unwrap();
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let db = Database::open(profile.path(), id).await.unwrap();
    let mut def = definition("fixture/row");
    def["fields"] = fixture["fields"].clone();
    db.register_projection_definition(serde_json::from_value(def).unwrap())
        .await
        .unwrap();
    let mut input = source("fixture/row", "fixture.md", "row");
    input["rows"][0]["data"] = fixture["data"].clone();
    db.replace_projection_source(serde_json::from_value(input).unwrap())
        .await
        .unwrap();
    db.close().await.unwrap();
    let mut actual=inspect(profile.path(),id,"SELECT field,ordinal,value_type,text_value,number_value,boolean_value,date_value,datetime_value FROM index_projection_values ORDER BY field,ordinal").await;
    for row in &mut actual {
        if let Some(n) = row[5].as_i64() {
            row[5] = json!(n != 0);
        }
    }
    let mut expected = fixture["expected"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_array().unwrap().clone())
        .collect::<Vec<_>>();
    expected.sort_by(|a, b| {
        a[0].as_str()
            .cmp(&b[0].as_str())
            .then(a[1].as_u64().cmp(&b[1].as_u64()))
    });
    // Turso returns REAL affinity as floating point; compare JSON numbers numerically.
    for (actual, expected) in actual.iter().zip(&expected) {
        for (left, right) in actual.iter().zip(expected) {
            if left.is_number() && right.is_number() {
                assert_eq!(left.as_f64(), right.as_f64());
            } else {
                assert_eq!(left, right);
            }
        }
    }
    assert_eq!(actual.len(), expected.len());
}

#[tokio::test]
async fn projection_replacement_accepts_64_rows_caps_fields_and_bounds_utf16_values() {
    let profile = tempfile::tempdir().unwrap();
    let id = Uuid::new_v4();
    let db = Database::open(profile.path(), id).await.unwrap();
    let mut def = definition("fixture/row");
    let fields = def["fields"].as_object_mut().unwrap();
    fields.clear();
    for n in 0..70 {
        fields.insert(format!("field{n}"), json!({"type":"string","indexed":true}));
    }
    for key in ["2", "1"] {
        fields.insert(key.into(), json!({"type":"string","indexed":true}));
    }
    let title = format!("{}🪨tail", "a".repeat(4094));
    let data = fields
        .keys()
        .map(|key| (key.clone(), json!(title)))
        .collect::<serde_json::Map<_, _>>();
    db.register_projection_definition(serde_json::from_value(def).unwrap())
        .await
        .unwrap();
    db.replace_projection_source(
        serde_json::from_value(source("fixture/row", "fixture.md", "old")).unwrap(),
    )
    .await
    .unwrap();
    let input = json!({"projectionId":"fixture/row","sourcePath":"fixture.md","sourceHash":"new","rows":(0..64).map(|i|json!({"id":format!("new-{i}"),"kind":"fixture","data":data})).collect::<Vec<_>>()});
    db.replace_projection_source(serde_json::from_value(input).unwrap())
        .await
        .unwrap();
    db.close().await.unwrap();
    assert_eq!(
        inspect(
            profile.path(),
            id,
            "SELECT COUNT(*) FROM index_projection_rows"
        )
        .await[0][0],
        json!(64)
    );
    assert_eq!(
        inspect(
            profile.path(),
            id,
            "SELECT COUNT(*) FROM index_projection_edges"
        )
        .await[0][0],
        json!(0)
    );
    let values = inspect(
        profile.path(),
        id,
        "SELECT field,text_value FROM index_projection_values WHERE row_id='new-0' ORDER BY field",
    )
    .await;
    assert_eq!(values.len(), 64);
    assert!(values.iter().any(|v| v[0] == json!("1")));
    assert!(values.iter().any(|v| v[0] == json!("2")));
    assert!(values.iter().any(|v| v[0] == json!("field61")));
    assert!(!values.iter().any(|v| v[0] == json!("field62")));
    assert!(
        values
            .iter()
            .all(|v| v[1] == json!(format!("{}🪨", "a".repeat(4094))))
    );
}
