use super::{Provider, fingerprint};
use lapis_app_protocol::database::search::embedding::SearchEmbeddingProviderKind as Kind;
use lapis_app_protocol::database::search::records::{
    SearchChunkEmbedding, SearchDocumentRecord, SearchEmbeddingStatus as Status,
};

impl Provider {
    pub(in crate::database) fn pending(
        &self,
        document: &mut SearchDocumentRecord,
        config: &lapis_app_protocol::database::search::embedding::SearchEmbeddingProviderConfig,
    ) {
        for chunk in document
            .chunks
            .iter_mut()
            .flatten()
            .filter(|chunk| !fingerprint::normalized(&chunk.text).is_empty())
        {
            chunk.embedding = Some(SearchChunkEmbedding {
                status: Status::Pending,
                model_id: config
                    .model_id
                    .clone()
                    .unwrap_or_else(|| "lapis/token-hash-v0".into()),
                model_version: config.model_version.clone().or_else(|| {
                    if config.kind == Kind::TokenHash {
                        Some("0".into())
                    } else {
                        config.revision.clone()
                    }
                }),
                dimensions: config
                    .dimensions
                    .or_else(|| (config.kind == Kind::TokenHash).then_some(48))
                    .map(|n| n as f64),
                vector: None,
                fingerprint: None,
                dirty: Some(true),
                updated_at: None,
                error: None,
            });
        }
    }
    pub(in crate::database) async fn prepare(
        &self,
        mut document: SearchDocumentRecord,
    ) -> SearchDocumentRecord {
        let Some(config) = &self.config else {
            return document;
        };
        let texts: Vec<_> = document
            .chunks
            .iter()
            .flatten()
            .filter(|chunk| !fingerprint::normalized(&chunk.text).is_empty())
            .map(|chunk| chunk.text.clone())
            .collect();
        if texts.is_empty() {
            return document;
        }
        self.pending(&mut document, config);
        let result = self.vectors(texts).await;
        let mut vectors = result.as_ref().ok().into_iter().flatten();
        let timestamp = super::now();
        for chunk in document
            .chunks
            .iter_mut()
            .flatten()
            .filter(|chunk| !fingerprint::normalized(&chunk.text).is_empty())
        {
            let embedding = chunk.embedding.as_mut().unwrap();
            embedding.updated_at = Some(timestamp);
            match &result {
                Ok(_) => {
                    let vector = vectors.next().unwrap();
                    embedding.status = Status::Ready;
                    embedding.dimensions = Some(vector.len() as f64);
                    embedding.vector = Some(vector.clone());
                    embedding.dirty = Some(false);
                    embedding.fingerprint = Some(fingerprint::fingerprint(&chunk.text));
                }
                Err(error) => {
                    embedding.status = Status::Error;
                    embedding.error = Some(error.to_string());
                }
            }
        }
        document
    }
}
