mod contracts;
pub use contracts::Contracts;
mod database;
mod editing_intents;
mod history_index;
mod indexing;
mod native_tasks;
mod participants;
mod queues;
mod repository_backup;
mod repository_restore;
mod repository_sources;
mod turns;
use lapis_app_protocol::database::call::*;
use lapis_app_protocol::database::indexed::{mutations::IndexedFile, queries::*, records::*};
use lapis_app_protocol::database::memory::{enums::*, jobs::*, records::*};
use lapis_app_protocol::database::projections::queries::*;
use lapis_app_protocol::database::projections::*;
use lapis_app_protocol::database::search::embedding::*;
use lapis_app_protocol::database::search::enhancement::*;
use lapis_app_protocol::database::search::retrieval::*;
use lapis_app_protocol::database::search::{records::*, source::*, *};
use lapis_app_protocol::database::tasks::{enums::*, record::*, recurrence::*, structure::*, *};
use lapis_app_protocol::database::{changes::*, history::*, state::*};
use lapis_app_protocol::turns::*;
use lapis_app_protocol::{browser::*, database::*, files::*, *};
use serde_json::json;
use ts_rs::{Config, TS};

fn main() {
    let config = Config::default();
    let declarations = [
        DatabaseMethod::decl(&config),
        DatabaseValue::decl(&config),
        DatabaseCall::decl(&config),
        LoginRequest::decl(&config),
        SessionInfo::decl(&config),
        HostInfo::decl(&config),
        LogoutReply::decl(&config),
        VaultName::decl(&config),
        MetaCheck::decl(&config),
        MetaMutation::decl(&config),
        MetaTransaction::decl(&config),
        MetaScan::decl(&config),
        MetaEntry::decl(&config),
        MetaPage::decl(&config),
        NotebookPhase::decl(&config),
        NotebookCell::decl(&config),
        NotebookState::decl(&config),
        SnapshotFile::decl(&config),
        MetadataSnapshot::decl(&config),
        NotificationSeverity::decl(&config),
        NotificationRecord::decl(&config),
        ChangeDomain::decl(&config),
        ChangedPath::decl(&config),
        ChangeSet::decl(&config),
        HistoryEvent::decl(&config),
        HistoryFile::decl(&config),
        HistoryRevision::decl(&config),
        FileHistory::decl(&config),
        StoreHistory::decl(&config),
        StoredHistory::decl(&config),
        FileRecord::decl(&config),
        MetadataRecord::decl(&config),
        LinkType::decl(&config),
        LinkKind::decl(&config),
        LinkRecord::decl(&config),
        TagRecord::decl(&config),
        PropertyRecord::decl(&config),
        CoreIndexedFile::decl(&config),
        IndexedFile::decl(&config),
        IndexedRow::decl(&config),
        ManifestPage::decl(&config),
        MetadataPage::decl(&config),
        MetadataScalar::decl(&config),
        PropertyOp::decl(&config),
        PropertyFilter::decl(&config),
        FileSortField::decl(&config),
        MetadataSortField::decl(&config),
        SortDirection::decl(&config),
        MetadataSort::decl(&config),
        MetadataQuery::decl(&config),
        ManifestQuery::decl(&config),
        MetadataDomain::decl(&config),
        MetadataPageQuery::decl(&config),
        FacetKind::decl(&config),
        FacetQuery::decl(&config),
        ScalarKind::decl(&config),
        FacetRow::decl(&config),
        LinkDirection::decl(&config),
        LinkResolution::decl(&config),
        LinkQuery::decl(&config),
        ProjectionFieldType::decl(&config),
        ProjectionVisibility::decl(&config),
        ProjectionCardinality::decl(&config),
        ProjectionField::decl(&config),
        ProjectionDefinition::decl(&config),
        ProjectionRow::decl(&config),
        ProjectionEdge::decl(&config),
        ProjectionSource::decl(&config),
        ProjectionSourceError::decl(&config),
        ProjectionComparison::decl(&config),
        ProjectionFilter::decl(&config),
        ProjectionSortDirection::decl(&config),
        ProjectionNulls::decl(&config),
        ProjectionOrder::decl(&config),
        ProjectionQuery::decl(&config),
        ProjectionRelationDirection::decl(&config),
        ProjectionRelatedQuery::decl(&config),
        ProjectionIndexStatus::decl(&config),
        ProjectionQueryResult::decl(&config),
        MemoryScope::decl(&config),
        MemoryOrigin::decl(&config),
        MemorySourceStatus::decl(&config),
        MemoryKind::decl(&config),
        MemoryCandidateState::decl(&config),
        MemoryJobKind::decl(&config),
        MemoryJobStatus::decl(&config),
        MemorySource::decl(&config),
        MemoryCandidate::decl(&config),
        MemoryCandidateOrigin::decl(&config),
        MemoryCandidateInput::decl(&config),
        MemoryCandidateQuery::decl(&config),
        MemoryRecallSignal::decl(&config),
        MemoryJob::decl(&config),
        MemoryJobPatch::decl(&config),
        MemoryJobClaim::decl(&config),
        MemoryJobUpdate::decl(&config),
        TaskKind::decl(&config),
        TaskStatus::decl(&config),
        TaskStartKind::decl(&config),
        TaskPlanKind::decl(&config),
        TaskOccurrenceState::decl(&config),
        TaskView::decl(&config),
        TaskRepeatAnchor::decl(&config),
        TaskRepeatMissed::decl(&config),
        TaskStructuralKind::decl(&config),
        TaskPriority::decl(&config),
        TaskRecord::decl(&config),
        TaskRepeat::decl(&config),
        TaskPauseRange::decl(&config),
        TaskTracking::decl(&config),
        TaskStructuralEntry::decl(&config),
        TaskRun::decl(&config),
        TaskStructureDiagnostic::decl(&config),
        TaskStructure::decl(&config),
        TaskQuery::decl(&config),
        TaskLookup::decl(&config),
        TaskChildQuery::decl(&config),
        SearchDocumentRecord::decl(&config),
        SearchDocumentChunk::decl(&config),
        SearchChunkKind::decl(&config),
        SearchEmbeddingStatus::decl(&config),
        SearchChunkEmbedding::decl(&config),
        SearchSourceMetadata::decl(&config),
        SearchChunkingSettings::decl(&config),
        SearchSourcePosition::decl(&config),
        SearchSourceRange::decl(&config),
        SearchSourceHeading::decl(&config),
        SearchSourceSection::decl(&config),
        SearchManifestQuery::decl(&config),
        SearchManifestRecord::decl(&config),
        SearchManifestPage::decl(&config),
        SearchIndexStats::decl(&config),
        SearchEmbeddingProviderKind::decl(&config),
        SearchEmbeddingProviderConfig::decl(&config),
        SearchEmbeddingDevice::decl(&config),
        SearchEmbeddingPooling::decl(&config),
        SearchEmbeddingRuntimePhase::decl(&config),
        SearchEmbeddingRuntimeStatus::decl(&config),
        SearchMode::decl(&config),
        SearchEnhancementRuntime::decl(&config),
        SearchEnhancementProviderKind::decl(&config),
        SearchEnhancementProvider::decl(&config),
        SearchEnhancementRequest::decl(&config),
        SearchEnhancementSignal::decl(&config),
        SearchEnhancementSkipReason::decl(&config),
        SearchEnhancementDiagnostics::decl(&config),
        SearchRetrievalMode::decl(&config),
        SearchField::decl(&config),
        SearchOptions::decl(&config),
        SearchRange::decl(&config),
        SearchSnippet::decl(&config),
        SearchFusionAlgorithm::decl(&config),
        SearchFusion::decl(&config),
        SearchScore::decl(&config),
        SearchDiagnostics::decl(&config),
        SearchResult::decl(&config),
        ManagedVault::decl(&config),
        AttachClient::decl(&config),
        FileOptions::decl(&config),
        FileCommand::decl(&config),
        EntryStat::decl(&config),
        FileListing::decl(&config),
        FileValue::decl(&config),
        ExpectedVersion::decl(&config),
        VaultCommand::decl(&config),
        Request::decl(&config),
        ClientDescriptor::decl(&config),
        VaultPhase::decl(&config),
        VaultStatus::decl(&config),
        EditorClaim::decl(&config),
        TextSnapshot::decl(&config),
        VaultValue::decl(&config),
        ErrorCode::decl(&config),
        ServiceError::decl(&config),
        Outcome::decl(&config),
        Reply::decl(&config),
    ];
    let typescript = format!(
        "// Generated by pnpm protocol:generate. Do not edit.\nexport const PROTOCOL_MAJOR = {PROTOCOL_MAJOR} as const;\n{}\n",
        history_index::declarations(&config)
            .iter()
            .chain(repository_backup::declarations(&config).iter())
            .chain(editing_intents::declarations(&config).iter())
            .chain(repository_restore::declarations(&config).iter())
            .chain(repository_sources::declarations(&config).iter())
            .chain(indexing::declarations(&config).iter())
            .chain(native_tasks::declarations(&config).iter())
            .chain(turns::declarations(&config).iter())
            .chain(queues::declarations(&config).iter())
            .chain(participants::declarations(&config).iter())
            .chain(declarations.iter())
            .map(|item| format!("export {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    let mut schema = schemars::schema_for!(Contracts).to_value();
    schema.sort_all_objects();
    let database_typescript = database::typescript(&config);
    println!(
        "{}",
        json!({"schema": schema, "typescript": typescript, "databaseTypescript": database_typescript, "turnTypescript": turns::typescript(&config), "queueTypescript": queues::typescript(&config), "participantTypescript": participants::typescript(&config), "indexingTypescript": indexing::typescript(&config), "historyIndexTypescript": history_index::typescript(&config), "tasksTypescript": native_tasks::typescript(&config), "editingIntentTypescript": editing_intents::typescript(&config)})
    );
}
