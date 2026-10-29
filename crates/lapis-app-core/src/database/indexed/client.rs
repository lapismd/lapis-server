use super::{Action, Value};
use crate::database::{Database, Operation, Output, invalid};
use crate::services::Result;
use lapis_app_protocol::database::indexed::{queries::*, records::*};

impl Database {
    /// Core file metadata only; plugin projections commit separately.
    pub async fn upsert_indexed_metadata(&self, record: CoreIndexedFile) -> Result<()> {
        validate_record(&record)?;
        self.submit(Operation::Indexed(Action::Put(Box::new(record))))
            .await
            .map(|_| ())
    }
    pub async fn get_indexed_file(&self, path: &str) -> Result<Option<IndexedRow>> {
        match self
            .submit(Operation::Indexed(Action::Get(path.into())))
            .await?
        {
            Output::Indexed(Value::File(value)) => Ok(value.map(|v| *v)),
            _ => unreachable!("worker output matches operation"),
        }
    }
    pub async fn list_indexed_file_manifest(&self, query: ManifestQuery) -> Result<ManifestPage> {
        match self
            .submit(Operation::Indexed(Action::Manifest(query)))
            .await?
        {
            Output::Indexed(Value::Manifest(page)) => Ok(page),
            _ => unreachable!("worker output matches operation"),
        }
    }
    pub async fn query_indexed_metadata(&self, query: MetadataQuery) -> Result<Vec<IndexedRow>> {
        let limit = query.limit.unwrap_or(10_000);
        self.query_indexed_metadata_page(MetadataPageQuery {
            query: Some(query),
            limit: Some(limit),
            ..Default::default()
        })
        .await
        .map(|p| p.rows)
    }
    pub async fn query_indexed_metadata_page(
        &self,
        query: MetadataPageQuery,
    ) -> Result<MetadataPage> {
        if query.query.as_ref().is_some_and(|q| {
            q.property_filters
                .iter()
                .any(|f| matches!(f.value,Some(MetadataScalar::Number(n)) if !n.is_finite()))
        }) {
            return Err(invalid("Invalid indexed query number"));
        }
        match self.submit(Operation::Indexed(Action::Page(query))).await? {
            Output::Indexed(Value::Page(page)) => Ok(page),
            _ => unreachable!("worker output matches operation"),
        }
    }
    pub async fn query_metadata_facets(&self, query: FacetQuery) -> Result<Vec<FacetRow>> {
        match self
            .submit(Operation::Indexed(Action::Facets(query)))
            .await?
        {
            Output::Indexed(Value::Facets(rows)) => Ok(rows),
            _ => unreachable!("worker output matches operation"),
        }
    }
    pub async fn query_metadata_links(&self, query: LinkQuery) -> Result<Vec<LinkRecord>> {
        match self
            .submit(Operation::Indexed(Action::Links(query)))
            .await?
        {
            Output::Indexed(Value::Links(rows)) => Ok(rows),
            _ => unreachable!("worker output matches operation"),
        }
    }
}

pub(super) fn validate_record(record: &CoreIndexedFile) -> Result<()> {
    let path = &record.file.path;
    if !record.file.mtime.is_finite()
        || !record.file.size.is_finite()
        || record.metadata.path != *path
        || record.links.iter().any(|l| {
            l.source_path != *path
                || !l.count.is_finite()
                || l.ordinal.is_some_and(|n| !n.is_finite())
        })
        || record.tags.iter().any(|t| t.path != *path)
        || record.properties.iter().any(|p| p.path != *path)
    {
        return Err(invalid(
            "Invalid core indexed file identity or numeric data",
        ));
    }
    Ok(())
}
