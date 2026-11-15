//! Read an admitted original or an existing recorded backup; never snapshot.
mod backup;
use super::source_access::SourceAccess;
use crate::services::{Client, Error, Result};
use jj_history_service::local::{self, Access as _};
use lapis_app_protocol::{ErrorCode, repository_remotes::RemoteRepositoryTarget as Target};
use std::sync::Arc;

fn unavailable() -> Error {
    Error::new(
        ErrorCode::Unsupported,
        "Repository working copy status is unavailable",
    )
}
impl Client {
    pub async fn repository_working_copy(&self) -> Result<String> {
        self.vault.lock()?.require_client(self.id)?;
        let work = self.vault.begin_work()?;
        let turn = self.vault.backup.reserve()?;
        let vault = self.vault.clone();
        let result = turn
            .run(work, move || {
                let original = super::policy::Policy::original(&vault)
                    .map_err(|_| unavailable())?
                    .is_some();
                let target = if original {
                    Target::Original
                } else {
                    Target::Backup
                };
                let access = SourceAccess::read(&vault, target).map_err(|_| unavailable())?;
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?;
                let result = if original {
                    runtime.block_on(read(access))
                } else {
                    let binding = vault
                        .backup_storage
                        .as_ref()
                        .ok_or_else(unavailable)?
                        .history_source(&vault)?
                        .binding;
                    runtime.block_on(backup::read(access, binding))
                };
                if super::policy::Policy::original(&vault)
                    .map_err(|_| unavailable())?
                    .is_some()
                    != original
                {
                    return Err(unavailable());
                }
                result
            })
            .await;
        self.vault.lock()?.require_client(self.id)?;
        result
    }
}
async fn read(access: Arc<SourceAccess>) -> Result<String> {
    let executor = local::Executor::new(1).map_err(|_| unavailable())?;
    let heads = executor
        .heads(access.clone())
        .await
        .map_err(|_| unavailable())?;
    let [operation] = heads.heads.as_slice() else {
        return Err(unavailable());
    };
    let observer =
        jj_store_service::working_tree::Observer::new(executor, 1).map_err(|_| unavailable())?;
    let result = observer
        .status(access.clone(), access.root().to_path_buf(), *operation)
        .await;
    observer.drain().await.map_err(|_| unavailable())?;
    let result = result.map_err(|_| unavailable())?;
    access.authorize().await.map_err(|_| unavailable())?;
    let author = result.commit.author;
    serde_json::to_string(&serde_json::json!({
        "version": 1, "source": "original", "operation": result.operation,
        "workspace": result.workspace, "working_copy": result.working_copy,
        "files": result.files, "lines": result.lines, "uncounted": result.uncounted,
        "commit": { "change_id": result.commit.change_id, "description": result.commit.description,
            "author": { "name": author.name, "email": author.email,
                "timestamp": { "millis": author.timestamp.millis.to_string(), "offset_minutes": author.timestamp.offset_minutes } }
        }
    })).map_err(|_| unavailable())
}
