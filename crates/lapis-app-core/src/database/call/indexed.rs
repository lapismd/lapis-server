use super::{Arguments, Database, done, optional, value};
use crate::services::Result;
use lapis_app_protocol::database::call::{DatabaseMethod, DatabaseMethod::*, DatabaseValue};

pub(super) async fn call(
    db: &Database,
    method: DatabaseMethod,
    a: &Arguments,
) -> Result<DatabaseValue> {
    match method {
        ConfigureSearchEmbeddingProvider => {
            done(db.configure_search_embedding_provider(a.required(0)?).await)
        }
        GetSearchEmbeddingProvider => value(db.get_search_embedding_provider().await),
        GetSearchEmbeddingRuntimeStatus => value(db.get_search_embedding_runtime_status()),
        GetSearchIndexStats => value(db.get_search_index_stats().await),
        UpsertIndexedFile => done(db.upsert_indexed_file(a.required(0)?).await),
        GetIndexedFile => optional(db.get_indexed_file(a.text(0)?).await),
        ListIndexedFileManifest => value(db.list_indexed_file_manifest(a.default(0)?).await),
        QueryIndexedMetadata => value(db.query_indexed_metadata(a.default(0)?).await),
        QueryIndexedMetadataPage => value(db.query_indexed_metadata_page(a.default(0)?).await),
        QueryMetadataFacets => value(db.query_metadata_facets(a.required(0)?).await),
        QueryMetadataLinks => value(db.query_metadata_links(a.required(0)?).await),
        DeleteIndexedFile => done(db.delete_indexed_file(a.text(0)?).await),
        RenameIndexedFile => done(db.rename_indexed_file(a.text(0)?, a.text(1)?).await),
        UpsertSearchDocument => done(db.upsert_search_document(a.required(0)?).await),
        DeleteSearchDocument => done(db.delete_search_document(a.text(0)?).await),
        GetSearchDocument => optional(db.get_search_document(a.text(0)?).await),
        ListSearchDocumentManifest => value(db.list_search_document_manifest(a.default(0)?).await),
        ListSearchDocuments => value(db.list_search_documents().await),
        RebuildSearchIndex => done(db.rebuild_search_index().await),
        SearchDocuments => value(db.search_documents(a.text(0)?, a.default(1)?).await),
        SearchDocumentPaths => value(db.search_document_paths(a.text(0)?, a.default(1)?).await),
        _ => unreachable!("call router selects the indexed domain"),
    }
}
