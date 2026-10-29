use super::{IndexingStatus, VaultService, unavailable};
use crate::services::{Client, Error, Result};
use lapis_app_protocol::{
    ErrorCode,
    indexing::{
        ContributionPlacement, ContributionStatus, IndexingCommand, IndexingValue,
        ProjectionContribution,
    },
};
use std::sync::Arc;

impl Client {
    /// One admitted operation; response cancellation never cancels selection or drain.
    pub async fn control_indexing(&self, command: IndexingCommand) -> Result<IndexingValue> {
        self.vault.lock()?.require_client(self.id)?;
        let work = self.vault.begin_work()?;
        let vault = self.vault.clone();
        tokio::spawn(async move {
            let _work = work;
            vault.control_indexing(command).await
        })
        .await
        .map_err(|_| unavailable())?
    }
}
impl VaultService {
    async fn control_indexing(self: &Arc<Self>, command: IndexingCommand) -> Result<IndexingValue> {
        if matches!(command, IndexingCommand::Status {}) {
            return Ok(IndexingValue {
                status: self.observed_indexing_status()?,
                contribution: None,
            });
        }
        let _control = self.indexing_control.lock().await;
        let mut contribution = None;
        let mut settled = None;
        match command {
            IndexingCommand::SelectMetadata {} => self.start_markdown_indexing().await?,
            IndexingCommand::SelectSearch { selection } => {
                contribution = Some(self.select_search_sources(selection).await?);
            }
            IndexingCommand::DisableSearch {} => self.disable_search_sources().await?,
            IndexingCommand::RegisterProcessor {
                owner_plugin_id,
                extension,
            } => {
                if owner_plugin_id.len() > 128
                    || extension.len() > 32
                    || owner_plugin_id.is_empty()
                    || extension.is_empty()
                {
                    return Err(Error::new(
                        ErrorCode::InvalidRequest,
                        "Invalid bounded metadata processor",
                    ));
                }
                let native = owner_plugin_id == "markdown"
                    && matches!(extension.as_str(), "md" | "markdown");
                if native {
                    self.start_markdown_indexing().await?;
                }
                contribution = Some(ContributionStatus {
                    projection_id: format!("processor:{owner_plugin_id}/{extension}"),
                    placement: if native {
                        ContributionPlacement::Native
                    } else {
                        ContributionPlacement::ClientDependent
                    },
                    message: (!native).then(|| {
                        "This processor requires a client; no native callback is available".into()
                    }),
                });
            }
            IndexingCommand::RegisterProjection {
                contribution: value,
            } => {
                contribution = Some(self.contribute(value).await?);
            }
            IndexingCommand::DisableProjection { projection_id } => {
                if projection_id.is_empty() || projection_id.len() > 128 {
                    return Err(Error::new(
                        ErrorCode::InvalidRequest,
                        "Invalid bounded projection identity",
                    ));
                }
                match projection_id.as_str() {
                    "tasks/task" => self.set_task_document_indexing(false).await?,
                    "tasks/occurrence" => self.set_task_occurrence_indexing(false).await?,
                    "docs/note" => self.stop_docs_indexing()?,
                    _ => {
                        return Err(Error::new(
                            ErrorCode::Unsupported,
                            "Projection is not compiled natively",
                        ));
                    }
                }
                // Wait for a pass admitted after the flag changed. Any previous
                // multi-stage publication is complete before definition removal.
                self.refresh_indexing(false).await?;
                let database = self.database().await?;
                database
                    .unregister_projection_definition(&projection_id)
                    .await?;
                database.release_native_projection(&projection_id).await;
            }
            IndexingCommand::Refresh { rebuild } => {
                settled = Some(self.refresh_indexing_snapshot(rebuild).await?.progress);
            }
            IndexingCommand::Status {} => {}
        }
        let status = match settled {
            Some(status) => Some(status),
            None => self.observed_indexing_status()?,
        };
        Ok(IndexingValue {
            status,
            contribution,
        })
    }
    fn observed_indexing_status(
        &self,
    ) -> Result<Option<lapis_app_protocol::indexing::IndexingStatus>> {
        match self.indexing_status() {
            Ok(status) => Ok(Some(status.progress)),
            Err(error) if error.code() == ErrorCode::Unsupported => Ok(None),
            Err(error) => Err(error),
        }
    }
    pub(super) async fn refresh_indexing(&self, rebuild: bool) -> Result<()> {
        self.refresh_indexing_snapshot(rebuild).await.map(|_| ())
    }
    async fn refresh_indexing_snapshot(&self, rebuild: bool) -> Result<IndexingStatus> {
        let refresh = self
            .indexing
            .lock()
            .map_err(|_| unavailable())?
            .as_ref()
            .ok_or_else(|| Error::new(ErrorCode::Unsupported, "Native indexing is not selected"))?
            .refresh(rebuild);
        refresh.wait().await
    }
    async fn contribute(
        self: &Arc<Self>,
        value: ProjectionContribution,
    ) -> Result<ContributionStatus> {
        if value.projection_id.is_empty()
            || value.projection_id.len() > 128
            || value.owner_plugin_id.is_empty()
            || value.owner_plugin_id.len() > 128
            || value.configuration_hash.len() > 1024
            || value
                .path_prefix
                .as_ref()
                .is_some_and(|path| path.len() > 1024)
            || !value.schema_version.is_finite()
        {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "Invalid bounded indexing contribution",
            ));
        }
        // Retained Tasks/Docs declare no opaque configuration hash. Native
        // cache identities belong to the compiled algorithm; an arbitrary
        // client hash is not evidence that its configured output is equivalent.
        let native =
            value.configuration_hash.is_empty()
                && match (value.owner_plugin_id.as_str(), value.projection_id.as_str()) {
                    ("tasks", "tasks/task")
                        if value.schema_version
                            == f64::from(lapis_task_services::TASK_PROJECTION_VERSION)
                            && value.path_prefix.is_none() =>
                    {
                        self.set_task_document_indexing(true).await?;
                        true
                    }
                    ("tasks", "tasks/occurrence")
                        if value.schema_version
                            == f64::from(
                                lapis_task_services::TASK_OCCURRENCE_PROJECTION_VERSION,
                            )
                            && value.path_prefix.is_none() =>
                    {
                        self.set_task_occurrence_indexing(true).await?;
                        true
                    }
                    ("docs", "docs/note")
                        if value.schema_version
                            == lapis_docs_projection::registration(&serde_json::json!("Docs"))
                                ["version"]
                                .as_f64()
                                .unwrap() =>
                    {
                        self.start_docs_indexing(value.path_prefix.as_deref().unwrap_or("docs"))
                            .await?;
                        true
                    }
                    _ => false,
                };
        Ok(ContributionStatus {
            projection_id: value.projection_id,
            placement: if native {
                ContributionPlacement::Native
            } else {
                ContributionPlacement::ClientDependent
            },
            message: (!native).then(|| {
                "This contribution requires a client; no native callback is available".into()
            }),
        })
    }
}

#[cfg(test)]
mod tests;
