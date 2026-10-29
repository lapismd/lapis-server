use super::{MetaMutation, MetaTransaction, encoded, engine, invalid, reads::parse, store::Store};
use crate::services::Result;
use lapis_app_protocol::database::{
    changes::ChangeDomain,
    state::{MetadataSnapshot, NotebookState},
};

pub(super) const SNAPSHOT_KEY: &str = "compat.metadataSnapshot";
pub(super) fn notebook_key(path: &str) -> String {
    format!("notebook.state:{path}")
}
impl Store {
    pub async fn save_notebook(
        &mut self,
        path: String,
        state: Option<NotebookState>,
    ) -> Result<bool> {
        let value = state
            .map(serde_json::to_value)
            .transpose()
            .map_err(|_| invalid("Invalid notebook state"))?;
        self.transact_domain(
            MetaTransaction {
                checks: vec![],
                mutations: vec![MetaMutation {
                    key: notebook_key(&path),
                    value,
                    delete: false,
                }],
            },
            ChangeDomain::Notebook,
            vec![path],
        )
        .await
    }
    pub async fn save_snapshot(&mut self, snapshot: MetadataSnapshot) -> Result<bool> {
        let value =
            serde_json::to_value(snapshot).map_err(|_| invalid("Invalid metadata snapshot"))?;
        self.transact_domain(
            MetaTransaction {
                checks: vec![],
                mutations: vec![MetaMutation {
                    key: SNAPSHOT_KEY.into(),
                    value: Some(value),
                    delete: false,
                }],
            },
            ChangeDomain::Metadata,
            vec![],
        )
        .await
    }
    pub async fn load_snapshot(&self) -> Result<Option<MetadataSnapshot>> {
        let stored = self.get(SNAPSHOT_KEY).await?;
        if let Some(value) = stored.filter(|v| !v.is_null()) {
            return serde_json::from_value(value)
                .map(Some)
                .map_err(|_| invalid("Invalid stored metadata snapshot"));
        }
        // Explicit deprecated operation only; never used by normal startup/indexing.
        let mut rows = self
            .connection
            .query("SELECT state_json FROM app_state WHERE id=1", ())
            .await
            .map_err(engine)?;
        let Some(row) = rows.next().await.map_err(engine)? else {
            return Ok(None);
        };
        let value = parse(row.get::<String>(0).map_err(engine)?)?;
        let snapshot = value
            .get("metadataSnapshot")
            .filter(|value| !value.is_null())
            .cloned();
        let result = snapshot
            .map(serde_json::from_value)
            .transpose()
            .map_err(|_| invalid("Invalid stored metadata snapshot"))?;
        encoded(&result)?;
        Ok(result)
    }
}
