use lapis_app_protocol::database::search::{enhancement::*, retrieval::*};

pub(super) fn resolve(
    options: &SearchOptions,
    signal: SearchEnhancementSignal,
    mode: SearchRetrievalMode,
) -> Option<SearchEnhancementDiagnostics> {
    let request = options.query_enhancement.as_ref()?;
    if !request.expand_query && !request.rerank_results {
        return None;
    }
    // Retain the existing provider capability and skip order, without adding
    // another model engine or treating a portable path as native authority.
    let skip_reason = if !matches!(
        request.runtime_target,
        SearchEnhancementRuntime::DenoDesktop
    ) {
        SearchEnhancementSkipReason::UnsupportedRuntime
    } else if signal.result_count > 0
        && (matches!(mode, SearchRetrievalMode::Lexical) && signal.top_lexical_score >= 25.
            || matches!(mode, SearchRetrievalMode::Hybrid)
                && signal.top_fused_score >= 1. / 61. + 1. / 62.)
    {
        SearchEnhancementSkipReason::StrongSignal
    } else {
        SearchEnhancementSkipReason::ProviderUnavailable
    };
    Some(SearchEnhancementDiagnostics {
        runtime_target: request.runtime_target,
        provider_kind: request.provider.kind,
        expand_query: request.expand_query,
        rerank_results: request.rerank_results,
        applied: false,
        skip_reason: Some(skip_reason),
        signal,
    })
}
