use super::{
    super::{query::Query, snippets},
    ranking::Entry,
};
use lapis_app_protocol::database::search::{
    embedding::SearchEmbeddingProviderConfig,
    records::{SearchDocumentRecord, SearchEmbeddingStatus},
    retrieval::*,
};
pub(super) struct Context<'a> {
    pub enhancement:
        Option<&'a lapis_app_protocol::database::search::enhancement::SearchEnhancementDiagnostics>,
    pub mode: SearchRetrievalMode,
    pub lexical_candidates: usize,
    pub vector_candidates: usize,
    pub config: Option<&'a SearchEmbeddingProviderConfig>,
}
pub(super) fn build(
    document: SearchDocumentRecord,
    entry: Entry,
    query: &Query,
    options: &SearchOptions,
    context: Context<'_>,
) -> SearchResult {
    let Context {
        enhancement,
        mode,
        lexical_candidates,
        vector_candidates,
        config,
    } = context;
    let snippets = snippets::semantic(&document, query, options, entry.lexical, &entry.chunks);
    let mut matched = vec![];
    for id in snippets
        .iter()
        .filter_map(|s| s.chunk_id.as_ref())
        .chain(&entry.chunks)
    {
        if !matched.contains(id) {
            matched.push(id.clone());
        }
    }
    let embedding = document
        .chunks
        .iter()
        .flatten()
        .filter_map(|c| c.embedding.as_ref())
        .find(|e| !e.model_id.is_empty());
    let diagnostics = options
        .include_diagnostics
        .unwrap_or(false)
        .then(|| SearchDiagnostics {
            query_enhancement: enhancement.cloned(),
            requested_mode: options.mode.unwrap_or_default(),
            applied_mode: mode,
            backend_kind: "turso-native".into(),
            lexical_candidate_count: lexical_candidates,
            vector_candidate_count: vector_candidates,
            total_candidate_count: lexical_candidates + vector_candidates,
            model_id: embedding
                .map(|e| e.model_id.clone())
                .or_else(|| config.and_then(|c| c.model_id.clone())),
            provider_kind: config.map(|c| c.kind),
            model_ready: config.is_some()
                || embedding.is_some_and(|e| matches!(e.status, SearchEmbeddingStatus::Ready)),
        });
    let score_breakdown = entry.score(mode);
    SearchResult {
        document,
        score: score_breakdown.fused,
        snippets,
        retrieval_mode: mode,
        score_breakdown,
        matched_chunk_ids: matched,
        diagnostics,
    }
}
