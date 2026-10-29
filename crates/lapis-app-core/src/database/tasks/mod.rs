mod client;
pub(super) use client::valid;
mod native_reads;
mod query;
mod reads;
mod writes;
use crate::database::store::Store;
use crate::services::Result;
use lapis_app_protocol::database::{
    indexed::records::LinkRecord,
    tasks::{TaskChildQuery, TaskLookup, TaskQuery, record::TaskRecord},
};
use serde::Serialize;
use serde_json::Value as Json;
const ID: &str = "tasks/task";
pub(crate) const NATIVE_CONFIG: &str = "native-tasks-document-v4-yaml12-1";
#[derive(Serialize)]
pub(super) enum Action {
    Put(Box<TaskRecord>),
    Delete(String),
    Query(TaskQuery),
    Get(TaskLookup),
    Children(TaskChildQuery),
    Descendants(String),
}
pub(super) enum Value {
    Done,
    Rows(Vec<Json>),
    Row(Option<Json>),
    Links(Vec<LinkRecord>),
}
impl Store {
    pub async fn tasks(&mut self, action: Action) -> Result<Value> {
        match action {
            Action::Put(row) => self.put_task(*row).await.map(|_| Value::Done),
            Action::Delete(path) => self.delete_task(&path).await.map(|_| Value::Done),
            Action::Query(query) => self
                .query_projection(ID, query::index_query(query), None)
                .await
                .map(|r| Value::Rows(r.rows)),
            Action::Get(lookup) => self.task_row(lookup).await.map(Value::Row),
            Action::Children(query) => self.task_children(query).await.map(Value::Links),
            Action::Descendants(path) => self.task_descendants(&path).await.map(Value::Rows),
        }
    }
}
