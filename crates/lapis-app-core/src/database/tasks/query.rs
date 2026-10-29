use lapis_app_protocol::database::{
    indexed::queries::MetadataScalar,
    projections::queries::{ProjectionComparison as C, ProjectionFilter as F, ProjectionQuery},
    tasks::{TaskQuery, enums::*},
};
fn cmp(field: &str, comparison: C, value: &str) -> F {
    F::Compare {
        field: field.into(),
        comparison,
        value: MetadataScalar::String(value.into()),
    }
}
fn eq(field: &str, value: &str) -> F {
    cmp(field, C::Eq, value)
}
fn null(field: &str) -> F {
    F::NotExists {
        field: field.into(),
    }
}
fn and<const N: usize>(filters: [F; N]) -> F {
    F::And {
        operands: filters.into(),
    }
}
fn or<const N: usize>(filters: [F; N]) -> F {
    F::Or {
        operands: filters.into(),
    }
}
pub(super) fn index_query(query: TaskQuery) -> ProjectionQuery {
    let mut clauses = vec![];
    if let Some(path) = query.document_path.filter(|s| !s.is_empty()) {
        clauses.push(eq("documentPath", &path));
    }
    if let Some(id) = query.document_id.filter(|s| !s.is_empty()) {
        clauses.push(eq("documentId", &id));
    }
    if let Some(kind) = query.kind {
        clauses.push(eq(
            "kind",
            match kind {
                TaskKind::Task => "task",
                TaskKind::TaskList => "task-list",
            },
        ));
    }
    if let Some(view) = query.view {
        let today = query
            .today
            .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d").to_string());
        let open = and([eq("kind", "task"), eq("status", "open")]);
        clauses.push(match view {
            TaskView::Inbox => and([
                open,
                F::Compare {
                    field: "inbox".into(),
                    comparison: C::Eq,
                    value: MetadataScalar::Boolean(true),
                },
            ]),
            TaskView::Today => and([
                open,
                or([
                    and([
                        or([
                            eq("effectiveOccurrenceState", "current"),
                            eq("effectiveOccurrenceState", "unsupported"),
                        ]),
                        eq("effectiveOccurrenceDate", &today),
                    ]),
                    and([
                        null("effectiveOccurrenceState"),
                        cmp("planDate", C::Lte, &today),
                    ]),
                    cmp("deadline", C::Lte, &today),
                ]),
            ]),
            TaskView::Anytime => and([
                open,
                null("planDate"),
                or([
                    eq("startKind", "anytime"),
                    and([eq("startKind", "date"), cmp("startDate", C::Lte, &today)]),
                ]),
            ]),
            TaskView::Upcoming => and([
                open,
                or([
                    and([
                        eq("effectiveOccurrenceState", "future"),
                        cmp("effectiveOccurrenceDate", C::Gt, &today),
                    ]),
                    and([
                        null("effectiveOccurrenceState"),
                        cmp("startDate", C::Gt, &today),
                    ]),
                    and([
                        null("effectiveOccurrenceState"),
                        cmp("planDate", C::Gt, &today),
                    ]),
                    cmp("deadline", C::Gt, &today),
                ]),
            ]),
            TaskView::Someday => and([open, eq("startKind", "someday")]),
            TaskView::Completed => eq("status", "completed"),
            TaskView::Review => and([
                open,
                or([
                    eq("effectiveOccurrenceState", "overdue"),
                    and([
                        null("effectiveOccurrenceState"),
                        cmp("planDate", C::Lt, &today),
                    ]),
                ]),
            ]),
        });
    }
    ProjectionQuery {
        r#where: if clauses.is_empty() {
            None
        } else {
            Some(F::And { operands: clauses })
        },
        limit: query.limit,
        ..Default::default()
    }
}
