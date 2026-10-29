use super::{
    MAX_BYTES, Output, changes::now_ms, encoded, engine, invalid, reads::parse, store::Store,
};
use crate::services::Result;
use lapis_app_protocol::database::{
    changes::{ChangeDomain, ChangeSet},
    state::NotificationRecord,
};
use serde::Serialize;

#[derive(Serialize)]
pub(super) enum Action {
    List,
    Upsert(NotificationRecord),
    Read(String),
    Clear(String),
    ClearAll,
}
impl Store {
    pub async fn notifications(&mut self, action: Action) -> Result<Output> {
        if matches!(action, Action::List) {
            let mut records = self.notification_rows().await?;
            records.retain(|record| !record.cleared);
            records.sort_by(|a, b| {
                b.created_at
                    .partial_cmp(&a.created_at)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            encoded(&records)?;
            return Ok(Output::Notifications(records));
        }
        self.begin().await?;
        let result = self.change_notifications(action).await;
        self.finish(result).await
    }
    async fn change_notifications(&self, action: Action) -> Result<(Output, Option<ChangeSet>)> {
        match action {
            Action::Upsert(record) => self.store_notification(record).await?,
            Action::Read(ref id) | Action::Clear(ref id) => {
                let mut rows = self
                    .connection
                    .query(
                        "SELECT data_json FROM notifications WHERE id=?",
                        [id.as_str()],
                    )
                    .await
                    .map_err(engine)?;
                let Some(row) = rows.next().await.map_err(engine)? else {
                    return Ok((Output::Done, None));
                };
                let mut record: NotificationRecord =
                    serde_json::from_value(parse(row.get::<String>(0).map_err(engine)?)?)
                        .map_err(|_| invalid("Invalid stored notification"))?;
                if record.id != *id {
                    return Err(invalid(
                        "Stored notification identity does not match its row",
                    ));
                }
                drop(rows);
                if matches!(action, Action::Read(_)) {
                    record.read = true;
                } else {
                    record.cleared = true;
                }
                record.updated_at = now_ms()? as f64;
                self.store_notification(record).await?;
            }
            Action::ClearAll => {
                let now = now_ms()? as f64;
                for mut record in self.notification_rows().await? {
                    record.cleared = true;
                    record.updated_at = now;
                    self.store_notification(record).await?;
                }
            }
            Action::List => unreachable!("read-only query handled before transaction"),
        }
        Ok((
            Output::Done,
            Some(self.journal(ChangeDomain::Notification, vec![]).await?),
        ))
    }
    async fn store_notification(&self, record: NotificationRecord) -> Result<()> {
        let json = String::from_utf8(encoded(&record)?)
            .map_err(|_| invalid("Invalid notification JSON"))?;
        self.connection.execute("INSERT INTO notifications(id,data_json) VALUES(?,?) ON CONFLICT(id) DO UPDATE SET data_json=excluded.data_json", turso::params![record.id,json]).await.map_err(engine)?;
        Ok(())
    }
    async fn notification_rows(&self) -> Result<Vec<NotificationRecord>> {
        let mut rows = self
            .connection
            .query("SELECT id,data_json FROM notifications", ())
            .await
            .map_err(engine)?;
        let mut records = Vec::new();
        let mut bytes = 0;
        while let Some(row) = rows.next().await.map_err(engine)? {
            let id = row.get::<String>(0).map_err(engine)?;
            let json = row.get::<String>(1).map_err(engine)?;
            bytes += json.len();
            if bytes >= MAX_BYTES {
                return Err(invalid("Notification result exceeds 32 MiB"));
            }
            let record: NotificationRecord = serde_json::from_value(parse(json)?)
                .map_err(|_| invalid("Invalid stored notification"))?;
            if record.id != id {
                return Err(invalid(
                    "Stored notification identity does not match its row",
                ));
            }
            records.push(record);
        }
        Ok(records)
    }
}
