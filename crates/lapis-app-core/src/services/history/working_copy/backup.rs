//! Immutable recorded checkpoint statistics. No physical scanner or diff engine.
use super::*;
use jj_store_jj::vault_workspace::HistoryReader;

pub(super) async fn read(access: Arc<SourceAccess>, binding: jj_vault::Binding) -> Result<String> {
    let executor = local::Executor::new(1).map_err(|_| unavailable())?;
    let heads = executor
        .heads(access.clone())
        .await
        .map_err(|_| unavailable())?;
    let [operation] = heads.heads.as_slice() else {
        return Err(unavailable());
    };
    let reader = HistoryReader::open(binding)
        .await
        .map_err(|_| unavailable())?;
    let checkpoint = reader
        .history_revision(None, 0)
        .await
        .map_err(|_| unavailable())?
        .revision
        .ok_or_else(unavailable)?
        .checkpoint;
    let commit_id = checkpoint.commit.parse().map_err(|_| unavailable())?;
    let local::Response::CommitSummary(object) = executor
        .read(
            access.clone(),
            *operation,
            local::Request::CommitSummary { commit: commit_id },
        )
        .await
        .map_err(|_| unavailable())?
    else {
        return Err(unavailable());
    };
    let local::ObjectData::Commit(commit) = object.data else {
        return Err(unavailable());
    };
    let [parent] = commit.parents.as_slice() else {
        return Err(unavailable());
    };
    let local::Response::Diff(diff) = executor
        .read(
            access.clone(),
            *operation,
            local::Request::Diff(local::diff::Request {
                from: *parent,
                to: commit_id,
                path: String::new(),
                content: true,
            }),
        )
        .await
        .map_err(|_| unavailable())?
    else {
        return Err(unavailable());
    };
    // The native query returns every changed path or refuses its complete
    // bounded response. Aggregate its exact line hunks; do not recompute diffs.
    let mut additions = 0usize;
    let mut deletions = 0usize;
    let mut uncounted = 0usize;
    for change in &diff.changes {
        match &change.content {
            local::diff::Content::Text { hunks } => {
                for hunk in hunks {
                    if hunk.kind == local::diff::HunkKind::Different {
                        additions = additions
                            .checked_add(hunk.after.lines().count())
                            .ok_or_else(unavailable)?;
                        deletions = deletions
                            .checked_add(hunk.before.lines().count())
                            .ok_or_else(unavailable)?;
                    }
                }
            }
            local::diff::Content::Unavailable { .. } => uncounted += 1,
        }
    }
    if executor
        .heads(access.clone())
        .await
        .map_err(|_| unavailable())?
        .heads
        != heads.heads
    {
        return Err(unavailable());
    }
    access.authorize().await.map_err(|_| unavailable())?;
    let author = commit.author;
    let lines =
        (uncounted == 0).then(|| serde_json::json!({"additions":additions,"deletions":deletions}));
    serde_json::to_string(&serde_json::json!({
        "version":1,"source":"backup","operation":operation,"workspace":"default",
        "working_copy":commit_id,"files":diff.changes.len(),"lines":lines,"uncounted":uncounted,
        "commit":{"change_id":commit.change_id,"description":commit.description,
            "author":{"name":author.name,"email":author.email,
                "timestamp":{"millis":author.timestamp.millis.to_string(),"offset_minutes":author.timestamp.offset_minutes}}}
    })).map_err(|_| unavailable())
}
