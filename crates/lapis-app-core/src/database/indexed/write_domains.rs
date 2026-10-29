use super::{
    normalize,
    writes::{enum_text, json},
};
use crate::database::{engine, invalid};
use crate::services::Result;
use lapis_app_protocol::database::indexed::records::{LinkRecord, PropertyRecord, TagRecord};
use serde_json::Value;
use std::collections::HashSet;

pub(super) async fn links(db: &turso::Connection, path: &str, links: &[LinkRecord]) -> Result<()> {
    for (ordinal, link) in links.iter().enumerate() {
        let mut data = link.clone();
        data.ordinal.get_or_insert(ordinal as f64);
        let json = json(&data)?;
        db.execute(
            "INSERT INTO links(source_path,ordinal,data_json) VALUES(?,?,?)",
            turso::params![path, ordinal as i64, json.as_str()],
        )
        .await
        .map_err(engine)?;
        let kind = link.kind.as_ref().map(enum_text).transpose()?;
        db.execute("INSERT INTO metadata_links(source_path,ordinal,normalized_target,resolved_target_path,resolution_state,link_type,link_kind,data_json) VALUES(?,?,?,?,?,?,?,?)",turso::params![path,ordinal as i64,normalize::trim(&link.target_text).to_lowercase(),link.resolved_target_path.clone(),if link.resolved_target_path.as_ref().is_some_and(|s|!s.is_empty()){"resolved"}else{"unresolved"},enum_text(&link.link_type)?,kind,json]).await.map_err(engine)?;
    }
    Ok(())
}
pub(super) async fn tags(db: &turso::Connection, path: &str, tags: &[TagRecord]) -> Result<()> {
    for (ordinal, tag) in tags.iter().enumerate() {
        let json = json(tag)?;
        db.execute(
            "INSERT INTO tags(path,ordinal,data_json) VALUES(?,?,?)",
            turso::params![path, ordinal as i64, json.as_str()],
        )
        .await
        .map_err(engine)?;
        db.execute("INSERT INTO metadata_tags(path,ordinal,tag,normalized_tag,data_json) VALUES(?,?,?,?,?)",turso::params![path,ordinal as i64,tag.tag.as_str(),normalize::tag(&tag.tag),json]).await.map_err(engine)?;
        let mut seen = HashSet::new();
        for ancestor in &tag.hierarchy {
            let ancestor = normalize::tag(ancestor);
            if seen.insert(ancestor.clone()) {
                db.execute("INSERT INTO metadata_tag_ancestors(path,tag_ordinal,ancestor,depth) VALUES(?,?,?,?)",turso::params![path,ordinal as i64,ancestor,(seen.len()-1) as i64]).await.map_err(engine)?;
            }
        }
    }
    Ok(())
}
pub(super) async fn properties(
    db: &turso::Connection,
    path: &str,
    properties: &[PropertyRecord],
) -> Result<()> {
    for (ordinal, property) in properties.iter().enumerate() {
        let json = json(property)?;
        let name = normalize::trim(&property.name).to_lowercase();
        db.execute(
            "INSERT INTO properties(path,ordinal,data_json) VALUES(?,?,?)",
            turso::params![path, ordinal as i64, json.as_str()],
        )
        .await
        .map_err(engine)?;
        db.execute("INSERT INTO metadata_properties(path,ordinal,name,normalized_name,inferred_type,declared_type,data_json) VALUES(?,?,?,?,?,?,?)",turso::params![path,ordinal as i64,property.name.as_str(),name.as_str(),property.inferred_type.as_str(),property.declared_type.clone(),json]).await.map_err(engine)?;
        let mut values = Vec::new();
        normalize::flatten(&property.value, property.name.clone(), &mut values);
        for (n, (property_path, value)) in values.into_iter().enumerate() {
            let (kind, text, number, boolean, date) = scalar_columns(value)?;
            db.execute("INSERT INTO metadata_property_values(path,property_ordinal,value_ordinal,property_name,property_path,normalized_property_path,value_type,text_value,number_value,boolean_value,date_value,data_json) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",turso::params![path,ordinal as i64,n as i64,name.as_str(),property_path.as_str(),normalize::property_path(&property_path),kind,text,number,boolean,date,super::writes::json(value)?]).await.map_err(engine)?;
        }
    }
    Ok(())
}
type Columns = (
    &'static str,
    Option<String>,
    Option<f64>,
    Option<i64>,
    Option<String>,
);
fn scalar_columns(value: &Value) -> Result<Columns> {
    Ok(match value {
        Value::Null => ("null", None, None, None, None),
        Value::Bool(v) => ("boolean", None, None, Some(i64::from(*v)), None),
        Value::Number(n) => (
            "number",
            None,
            Some(
                n.as_f64()
                    .ok_or_else(|| invalid("Invalid property number"))?,
            ),
            None,
            None,
        ),
        Value::String(s) if normalize::date(s) => ("date", None, None, None, Some(s.clone())),
        Value::String(s) => ("string", Some(s.clone()), None, None, None),
        _ => return Err(invalid("Indexed property was not flattened")),
    })
}
