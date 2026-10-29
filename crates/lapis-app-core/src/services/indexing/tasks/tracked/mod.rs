//! Proactive default-daily observations on the existing admitted metadata owner.
mod materialize;
mod selection;
pub use selection::{TrackedTaskError, TrackedTaskFailure, TrackedTaskProvider, TrackedTaskStatus};

use crate::{
    database::Database,
    services::{Error, Result, Work},
};
use lapis_app_protocol::{
    ErrorCode,
    database::{
        indexed::queries::MetadataScalar,
        projections::queries::{ProjectionComparison, ProjectionFilter, ProjectionQuery},
    },
};
use lapis_task_services::{ParsedTaskDocument, TASKS_PUBLIC_PROJECTION_ID};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

fn compare(field: &str, value: &str) -> ProjectionFilter {
    ProjectionFilter::Compare {
        field: field.into(),
        comparison: ProjectionComparison::Eq,
        value: MetadataScalar::String(value.into()),
    }
}
fn query(date: &str, after: Option<String>) -> ProjectionQuery {
    ProjectionQuery {
        r#where: Some(ProjectionFilter::And {
            operands: vec![
                compare("kind", "task"),
                compare("status", "open"),
                compare("effectiveOccurrenceState", "current"),
                compare("effectiveOccurrenceDate", date),
                ProjectionFilter::Exists {
                    field: "tracking".into(),
                },
            ],
        }),
        limit: Some(500.0),
        after,
        ..Default::default()
    }
}
pub(super) async fn run(
    work: &Work,
    database: &Database,
    documents: &[ParsedTaskDocument],
    stop: &AtomicBool,
    selected: &AtomicBool,
    providers: &Mutex<Option<TrackedTaskProvider>>,
    date: &str,
) -> Result<Option<TrackedTaskStatus>> {
    if providers.lock().map_err(|_| Error::busy())?.is_none() {
        return Ok(None);
    }
    let mut status = TrackedTaskStatus {
        date: date.into(),
        ..Default::default()
    };
    let documents: HashMap<_, _> = documents
        .iter()
        .map(|doc| (doc.path.as_str(), doc))
        .collect();
    let mut after = None;
    loop {
        if stop.load(Ordering::Acquire)
            || work.cancellation_requested()
            || !selected.load(Ordering::Acquire)
        {
            break;
        }
        let page = database
            .query_projection(TASKS_PUBLIC_PROJECTION_ID, query(date, after), None)
            .await?;
        for row in page.rows {
            if stop.load(Ordering::Acquire)
                || work.cancellation_requested()
                || !selected.load(Ordering::Acquire)
            {
                return Ok(Some(status));
            }
            let Some(provider) = providers.lock().map_err(|_| Error::busy())?.clone() else {
                return Ok(Some(status));
            };
            if row["tracking"].is_null() {
                continue;
            }
            let path = row["documentPath"].as_str().ok_or_else(|| {
                Error::new(
                    ErrorCode::InvalidRequest,
                    "Tracked Task projection has no source path",
                )
            })?;
            let document = documents
                .get(path)
                .filter(|doc| row["documentId"] == doc.id)
                .ok_or_else(|| {
                    Error::new(
                        ErrorCode::Conflict,
                        "Tracked Task projection is outside its admitted snapshot",
                    )
                })?;
            status.attempted += 1;
            match materialize::apply(work, document, date, &provider) {
                Ok(applied) => {
                    status.materialized += 1;
                    status.changed += usize::from(applied.changed);
                    // Keep note publication in the ordinary next Markdown pass.
                    drop(applied.write);
                }
                Err(cause) => status.failures.push(TrackedTaskFailure {
                    path: path.into(),
                    cause: Arc::new(cause),
                    retained_daily: work.locate_default_daily_document(date).ok().flatten(),
                }),
            }
        }
        after = page.next_cursor;
        if after.is_none() {
            break;
        }
    }
    Ok(Some(status))
}
