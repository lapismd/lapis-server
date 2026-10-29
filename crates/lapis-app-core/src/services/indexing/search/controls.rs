use super::{Policy, SearchIndexingConfiguration};
use crate::{
    database::Database,
    services::{Error, Result, VaultService},
};
use lapis_app_protocol::{
    ErrorCode,
    indexing::{ContributionPlacement, ContributionStatus, search::SearchIndexingSelection},
};
use std::{collections::BTreeSet, sync::Arc};

impl VaultService {
    pub(in crate::services::indexing) async fn select_search_sources(
        self: &Arc<Self>,
        selection: SearchIndexingSelection,
    ) -> Result<ContributionStatus> {
        Database::validate_search_chunking(&selection.chunking)?;
        let mut ids = BTreeSet::new();
        if selection.providers.len() > 64
            || selection.providers.iter().any(|provider| {
                provider.provider_id.is_empty()
                    || provider.provider_id.len() > 128
                    || provider.provider_id.trim() != provider.provider_id
                    || provider.version.len() > 128
                    || !provider.priority.is_finite()
                    || !ids.insert(provider.provider_id.clone())
            })
        {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "Invalid bounded Search provider registry",
            ));
        }
        let unsupported = selection
            .providers
            .iter()
            .filter(|provider| {
                provider.version != "1"
                    || !matches!(
                        provider.provider_id.as_str(),
                        "search:markdown" | "search:canvas"
                    )
            })
            .map(|provider| provider.provider_id.as_str())
            .collect::<Vec<_>>();
        if !unsupported.is_empty() {
            // Unknown match callbacks can override any path. Fence all compiled
            // Search work rather than silently indexing through another provider.
            self.database().await?.claim_native_search().await;
            self.stop_search_indexing()?;
            self.drain_search_selection().await?;
            return Ok(ContributionStatus {
                projection_id: "search".into(),
                placement: ContributionPlacement::ClientDependent,
                message: Some(format!(
                    "Search providers require a client: {}",
                    unsupported.join(", ")
                )),
            });
        }
        self.start_search_policy(Policy {
            markdown: ids.contains("search:markdown"),
            prune_unselected: true,
            options: SearchIndexingConfiguration {
                canvas: ids.contains("search:canvas"),
                chunking: selection.chunking,
            },
        })
        .await?;
        self.refresh_indexing(false).await?;
        Ok(ContributionStatus {
            projection_id: "search".into(),
            placement: ContributionPlacement::Native,
            message: None,
        })
    }

    pub(in crate::services::indexing) async fn disable_search_sources(
        self: &Arc<Self>,
    ) -> Result<()> {
        self.stop_search_indexing()?;
        self.drain_search_selection().await?;
        self.database().await?.release_native_search().await;
        Ok(())
    }

    async fn drain_search_selection(&self) -> Result<()> {
        match self.indexing_status() {
            Ok(_) => self.refresh_indexing(false).await,
            Err(error) if error.code() == ErrorCode::Unsupported => Ok(()),
            Err(error) => Err(error),
        }
    }
}
