use crate::host::{Host, Result, error};
use lapis_app_core::archive::{ArchiveLimits, ArchiveSnapshot};
use lapis_app_protocol::ErrorCode;
use std::sync::Arc;
use uuid::Uuid;

impl Host {
    pub(crate) async fn archive_snapshot(self: &Arc<Self>, id: Uuid) -> Result<ArchiveSnapshot> {
        let manager = self.management.get().ok_or_else(|| {
            error(
                ErrorCode::Unsupported,
                "Vault transfers require the installation profile",
            )
        })?;
        let accepted = manager.accept()?;
        let work = {
            let _admission = manager.catalogue.admission(id)?;
            self.runtime
                .get(id)
                .and_then(|vault| vault.begin_work())
                .map_err(|error| error.0)?
        };
        // The blocking task owns both leases even if its HTTP waiter disappears.
        tokio::task::spawn_blocking(move || {
            let _accepted = accepted;
            work.archive_snapshot(ArchiveLimits::default())
                .map_err(|error| error.0)
        })
        .await
        .map_err(|_| error(ErrorCode::Io, "Vault export snapshot is unavailable"))?
    }
}
