use super::{Arguments, Database, done, optional, value};
use crate::services::Result;
use lapis_app_protocol::database::call::{DatabaseMethod, DatabaseMethod::*, DatabaseValue};

pub(super) async fn call(
    db: &Database,
    method: DatabaseMethod,
    a: &Arguments,
) -> Result<DatabaseValue> {
    match method {
        UpsertTaskProjection => done(db.upsert_task_projection(a.required(0)?).await),
        DeleteTaskProjection => done(db.delete_task_projection(a.text(0)?).await),
        QueryTasks => value(db.query_tasks(a.default(0)?).await),
        GetTaskRow => optional(db.get_task_row(a.required(0)?).await),
        ListChildLinks => value(db.list_child_links(a.required(0)?).await),
        ListTaskDescendants => value(db.list_task_descendants(a.text(0)?).await),
        RegisterProjectionDefinition => {
            done(db.register_projection_definition(a.required(0)?).await)
        }
        UnregisterProjectionDefinition => {
            done(db.unregister_projection_definition(a.text(0)?).await)
        }
        ReplaceProjectionSource => done(db.replace_projection_source(a.required(0)?).await),
        MarkProjectionSourceError => done(db.mark_projection_source_error(a.required(0)?).await),
        DeleteProjectionSource => done(
            db.delete_projection_source(a.text(0)?, a.text(1)?, a.optional_text(2)?)
                .await,
        ),
        QueryProjection => value(
            db.query_projection(a.text(0)?, a.default(1)?, a.optional_text(2)?)
                .await,
        ),
        GetProjectionRow => value(
            db.get_projection_row(a.text(0)?, a.text(1)?, a.optional_text(2)?)
                .await,
        ),
        QueryRelated => value(db.query_related(a.required(0)?, a.optional_text(1)?).await),
        _ => unreachable!("call router selects the projection domain"),
    }
}
