use super::{Budget, filter_sql, limit, number, optional_text, slots, text};
use crate::database::{encoded, engine, invalid, reads::parse, store::Store};
use crate::services::Result;
use lapis_app_protocol::database::indexed::{queries::*, records::*};
use serde::{Serialize, de::DeserializeOwned};
use std::collections::{HashMap, HashSet};

impl Store {
    pub async fn indexed_manifest(&self, query: ManifestQuery) -> Result<ManifestPage> {
        let count = limit(query.limit, 500, 1)?;
        let mut args = vec![text(query.after.unwrap_or_default())];
        let paths: Vec<_> = query
            .paths
            .into_iter()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let restriction = if paths.is_empty() {
            String::new()
        } else {
            args.extend(paths.iter().cloned().map(text));
            format!(" AND f.path IN ({})", slots(paths.len()))
        };
        args.push(turso::Value::Integer((count + 1) as i64));
        let mut rows=self.connection.query(&format!("SELECT f.path,f.normalized_path,f.extension,f.mtime,f.size,f.hash,f.indexed,f.deleted,m.parser_version FROM files f LEFT JOIN metadata m ON m.path=f.path WHERE f.indexed=1 AND f.deleted=0 AND f.path>? {restriction} ORDER BY f.path LIMIT ?"),args).await.map_err(engine)?;
        let mut files = Vec::new();
        let mut budget = Budget::new();
        let mut next_cursor = None;
        while let Some(row) = rows.next().await.map_err(engine)? {
            if files.len() == count {
                next_cursor = files.last().map(|f: &FileRecord| f.path.clone());
                break;
            }
            if files.len() >= 100_000 {
                return Err(invalid("Indexed result exceeds 100,000 rows"));
            }
            let file = file_record(&row, true)?;
            budget.take(encoded(&file)?.len())?;
            files.push(file);
        }
        let page = ManifestPage {
            rows: files,
            next_cursor,
        };
        encoded(&page)?;
        Ok(page)
    }
    pub async fn indexed_page(&self, input: MetadataPageQuery) -> Result<MetadataPage> {
        let query = input.query.unwrap_or_default();
        let count = limit(input.limit.or(query.limit), 100, 1)?;
        let (sql, args) = filter_sql::paths(&query, input.after.as_deref(), count + 1);
        let mut rows = self.connection.query(&sql, args).await.map_err(engine)?;
        let mut paths = Vec::new();
        let mut next_cursor = None;
        let mut budget = Budget::new();
        while let Some(row) = rows.next().await.map_err(engine)? {
            if paths.len() == count {
                next_cursor = paths.last().cloned();
                break;
            }
            if paths.len() >= 100_000 {
                return Err(invalid("Indexed result exceeds 100,000 rows"));
            }
            let path = row.get::<String>(0).map_err(engine)?;
            budget.take(path.len())?;
            paths.push(path);
        }
        drop(rows);
        let page = MetadataPage {
            rows: self
                .read_indexed(&paths, input.include.as_deref(), true)
                .await?,
            next_cursor,
        };
        encoded(&page)?;
        Ok(page)
    }
    pub async fn read_indexed(
        &self,
        paths: &[String],
        include: Option<&[MetadataDomain]>,
        signature: bool,
    ) -> Result<Vec<IndexedRow>> {
        let has = |d| include.is_none_or(|domains| domains.contains(&d));
        let mut budget = Budget::new();
        let mut files = HashMap::new();
        let mut metadata = HashMap::new();
        let mut properties: HashMap<String, Vec<PropertyRecord>> = HashMap::new();
        let mut tags: HashMap<String, Vec<TagRecord>> = HashMap::new();
        let mut links: HashMap<String, Vec<LinkRecord>> = HashMap::new();
        for chunk in paths.chunks(400) {
            let placeholders = slots(chunk.len());
            let args: Vec<_> = chunk.iter().cloned().map(text).collect();
            let select = if signature { ",m.parser_version" } else { "" };
            let join = if signature {
                "LEFT JOIN metadata m ON m.path=f.path"
            } else {
                ""
            };
            let mut rows=self.connection.query(&format!("SELECT f.path,f.normalized_path,f.extension,f.mtime,f.size,f.hash,f.indexed,f.deleted{select} FROM files f {join} WHERE f.indexed=1 AND f.deleted=0 AND f.path IN ({placeholders})"),args.clone()).await.map_err(engine)?;
            while let Some(row) = rows.next().await.map_err(engine)? {
                let file = file_record(&row, signature)?;
                budget.take(encoded(&file)?.len())?;
                files.insert(file.path.clone(), file);
            }
            drop(rows);
            if has(MetadataDomain::Metadata) {
                let mut rows=self.connection.query(&format!("SELECT path,hash,parser_version,data_json FROM metadata WHERE path IN ({placeholders})"),args.clone()).await.map_err(engine)?;
                while let Some(row) = rows.next().await.map_err(engine)? {
                    let json = row.get::<String>(3).map_err(engine)?;
                    budget.take(json.len())?;
                    let record = MetadataRecord {
                        path: row.get(0).map_err(engine)?,
                        hash: row.get(1).map_err(engine)?,
                        parser_version: row.get(2).map_err(engine)?,
                        metadata: parse(json)?,
                    };
                    budget.take(
                        record
                            .path
                            .len()
                            .saturating_add(record.hash.len())
                            .saturating_add(record.parser_version.len()),
                    )?;
                    metadata.insert(record.path.clone(), record);
                }
            }
            if has(MetadataDomain::Properties) {
                read_domain(
                    &self.connection,
                    "metadata_properties",
                    "path",
                    &placeholders,
                    &args,
                    &mut properties,
                    &mut budget,
                )
                .await?;
            }
            if has(MetadataDomain::Tags) {
                read_domain(
                    &self.connection,
                    "metadata_tags",
                    "path",
                    &placeholders,
                    &args,
                    &mut tags,
                    &mut budget,
                )
                .await?;
            }
            if has(MetadataDomain::Links) {
                read_domain(
                    &self.connection,
                    "metadata_links",
                    "source_path",
                    &placeholders,
                    &args,
                    &mut links,
                    &mut budget,
                )
                .await?;
            }
        }
        let result = paths
            .iter()
            .filter_map(|path| {
                files.remove(path).map(|file| IndexedRow {
                    file,
                    metadata: metadata.remove(path),
                    properties: properties.remove(path).unwrap_or_default(),
                    tags: tags.remove(path).unwrap_or_default(),
                    links: links.remove(path).unwrap_or_default(),
                })
            })
            .collect::<Vec<_>>();
        encoded(&result)?;
        Ok(result)
    }
}
fn file_record(row: &turso::Row, signature: bool) -> Result<FileRecord> {
    Ok(FileRecord {
        path: row.get(0).map_err(engine)?,
        normalized_path: row.get(1).map_err(engine)?,
        extension: row.get(2).map_err(engine)?,
        mtime: number(row, 3)?,
        size: number(row, 4)?,
        hash: row.get(5).map_err(engine)?,
        indexed: row.get::<i64>(6).map_err(engine)? != 0,
        deleted: Some(row.get::<i64>(7).map_err(engine)? != 0),
        parser_version: if signature {
            optional_text(row, 8)?
        } else {
            None
        },
    })
}
async fn read_domain<T: DeserializeOwned + Serialize>(
    db: &turso::Connection,
    table: &str,
    column: &str,
    placeholders: &str,
    args: &[turso::Value],
    out: &mut HashMap<String, Vec<T>>,
    budget: &mut Budget,
) -> Result<()> {
    let mut rows=db.query(&format!("SELECT {column},data_json FROM {table} WHERE {column} IN ({placeholders}) ORDER BY {column},ordinal"),args.to_vec()).await.map_err(engine)?;
    while let Some(row) = rows.next().await.map_err(engine)? {
        let path = row.get::<String>(0).map_err(engine)?;
        let json = row.get::<String>(1).map_err(engine)?;
        budget.take(path.len().saturating_add(json.len()))?;
        let record = serde_json::from_value(parse(json)?)
            .map_err(|_| invalid("Invalid stored indexed domain"))?;
        out.entry(path).or_default().push(record);
    }
    Ok(())
}
