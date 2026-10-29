#![allow(dead_code)]
use serde_json::{Value, json};
use uuid::Uuid;

pub fn definition(id: &str) -> Value {
    json!({"projectionId":id,"ownerPluginId":id.split('/').next().unwrap(),"schemaVersion":2,"configHash":"config-2","visibility":"public","fields":{"title":{"type":"string","indexed":true},"score":{"type":"number","sortable":true},"done":{"type":"boolean","indexed":true},"tags":{"type":"path","indexed":true,"cardinality":"many"},"literal.key":{"type":"date","indexed":true}},"active":true,"updatedAt":10})
}
pub fn source(id: &str, path: &str, row_id: &str) -> Value {
    json!({"projectionId":id,"sourcePath":path,"sourceHash":"hash-1","writerPluginId":id.split('/').next().unwrap(),"rows":[{"id":row_id,"kind":"book","data":{"id":row_id,"title":"native 🪨","score":"42","done":"false","tags":["one",null,"two"],"literal.key":"2026-10-24","literal":{"key":"wrong"}}}],"edges":[{"sourceRowId":row_id,"relation":"reference","targetPath":"target.md","ordinal":0,"data":{"label":"link"}}]})
}
pub async fn inspect(profile: &std::path::Path, id: Uuid, sql: &str) -> Vec<Vec<Value>> {
    let path = profile.join(format!("{id}.db"));
    let db = turso::Builder::new_local(path.to_str().unwrap())
        .build()
        .await
        .unwrap();
    let connection = db.connect().unwrap();
    let mut rows = connection.query(sql, ()).await.unwrap();
    let mut result = vec![];
    while let Some(row) = rows.next().await.unwrap() {
        let mut record = vec![];
        for i in 0..row.column_count() {
            record.push(match row.get_value(i).unwrap() {
                turso::Value::Null => Value::Null,
                turso::Value::Integer(n) => json!(n),
                turso::Value::Real(n) => json!(n),
                turso::Value::Text(s) => json!(s),
                _ => panic!("Unexpected projection value"),
            });
        }
        result.push(record);
    }
    result
}
