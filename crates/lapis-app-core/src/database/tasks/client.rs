use super::{Action, Value as TaskValue};
use crate::database::{Database, Operation, Output, invalid};
use crate::services::Result;
use lapis_app_protocol::database::{
    indexed::records::LinkRecord,
    tasks::{TaskChildQuery, TaskLookup, TaskQuery, record::TaskRecord, recurrence::TaskTracking},
};
use serde_json::Value;
impl Database {
    pub async fn upsert_task_projection(&self, row: TaskRecord) -> Result<()> {
        if !valid(&row) {
            return Err(invalid("Invalid task numeric data"));
        }
        self.submit(Operation::Tasks(Action::Put(Box::new(row))))
            .await
            .map(|_| ())
    }
    pub async fn delete_task_projection(&self, path: &str) -> Result<()> {
        self.submit(Operation::Tasks(Action::Delete(path.into())))
            .await
            .map(|_| ())
    }
    pub async fn query_tasks(&self, query: TaskQuery) -> Result<Vec<Value>> {
        match self.submit(Operation::Tasks(Action::Query(query))).await? {
            Output::Tasks(TaskValue::Rows(rows)) => Ok(rows),
            _ => unreachable!("matching worker output"),
        }
    }
    pub async fn get_task_row(&self, lookup: TaskLookup) -> Result<Option<Value>> {
        match self.submit(Operation::Tasks(Action::Get(lookup))).await? {
            Output::Tasks(TaskValue::Row(row)) => Ok(row),
            _ => unreachable!("matching worker output"),
        }
    }
    pub async fn list_child_links(&self, query: TaskChildQuery) -> Result<Vec<LinkRecord>> {
        match self
            .submit(Operation::Tasks(Action::Children(query)))
            .await?
        {
            Output::Tasks(TaskValue::Links(rows)) => Ok(rows),
            _ => unreachable!("matching worker output"),
        }
    }
    pub async fn list_task_descendants(&self, path: &str) -> Result<Vec<Value>> {
        match self
            .submit(Operation::Tasks(Action::Descendants(path.into())))
            .await?
        {
            Output::Tasks(TaskValue::Rows(rows)) => Ok(rows),
            _ => unreachable!("matching worker output"),
        }
    }
}
pub(in crate::database) fn valid(row: &TaskRecord) -> bool {
    let mut numbers = vec![
        row.checklist_total,
        row.checklist_completed,
        row.comment_count,
        row.projection_version,
    ];
    numbers.extend(row.duration_minutes.flatten());
    numbers.extend(row.repeat_interval.flatten());
    if let Some(repeat) = row.repeat.as_ref().and_then(Option::as_ref) {
        numbers.extend(repeat.completion_count);
    }
    if let Some(TaskTracking::Count { target, .. } | TaskTracking::Number { target, .. }) =
        row.tracking.as_ref().and_then(Option::as_ref)
    {
        numbers.extend(*target);
    }
    if let Some(structure) = row.structure.as_ref().and_then(Option::as_ref) {
        numbers.push(structure.version);
        for e in &structure.entries {
            numbers.extend([e.position_in_run, e.source_start, e.source_end]);
            numbers.extend(e.heading_level);
        }
        for r in &structure.runs {
            numbers.extend([r.list_depth, r.start_offset, r.end_offset, r.entry_count]);
            numbers.extend(r.heading_level);
        }
        for d in &structure.diagnostics {
            numbers.extend(d.source_start);
            numbers.extend(d.source_end);
        }
    }
    numbers.into_iter().all(f64::is_finite)
}
