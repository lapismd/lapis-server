use super::{
    DailyDocumentOperationResult, DailyDocumentPlan, DailyDocumentSettings, DailyFormatContext,
    format, plan_daily_document, policy,
    snapshot::{self, Snapshot},
};
use crate::{
    services::{Error, Work, guards, operations, state::State},
    vault::sync_directory,
};
use lapis_app_protocol::{ErrorCode, ExpectedVersion, TextSnapshot, VaultPhase};

pub(in crate::services) struct Prepared {
    snapshot: Snapshot,
    plan: Option<DailyDocumentPlan>,
}
pub(in crate::services) fn prepare(
    work: &Work,
    date: &str,
    settings: Option<(&DailyDocumentSettings, &DailyFormatContext)>,
) -> DailyDocumentOperationResult<Prepared> {
    snapshot::admitted_root(work)?;
    format::date(date)?;
    let snapshot = Snapshot::read(work)?;
    let plan = match settings {
        Some((settings, context)) => {
            plan_daily_document(date, true, &snapshot.candidates, settings, context)?
        }
        None => policy::locate(date, &snapshot.candidates)?,
    };
    Ok(Prepared { snapshot, plan })
}
impl Prepared {
    pub(in crate::services) fn plan(&self) -> &Option<DailyDocumentPlan> {
        &self.plan
    }
    pub(in crate::services) fn existing(
        &self,
        path: &str,
    ) -> crate::services::Result<TextSnapshot> {
        self.snapshot.existing(path)
    }
}
fn parents(state: &State, folder: &str) -> crate::services::Result<()> {
    guards::write(state, folder, None, false)?;
    state.root.mkdir(folder, true)?;
    let mut handles = vec![state.root.directory_handle()?];
    let mut path = String::new();
    for part in folder.split('/') {
        if !path.is_empty() {
            path.push('/');
        }
        path.push_str(part);
        handles.push(state.root.open_directory(&path)?);
    }
    for handle in handles.iter().rev() {
        sync_directory(handle)?;
    }
    Ok(())
}
pub(in crate::services) fn finish(
    work: &Work,
    prepared: Prepared,
) -> DailyDocumentOperationResult<Option<TextSnapshot>> {
    let state = work.vault.lock()?;
    if state.phase == VaultPhase::Closed {
        return Err(Error::closed().into());
    }
    prepared.snapshot.verify(&state)?;
    match prepared.plan {
        None => Ok(None),
        Some(DailyDocumentPlan::Existing { path }) => Ok(Some(prepared.snapshot.existing(&path)?)),
        Some(DailyDocumentPlan::Create { path, text }) => {
            if path.split('/').any(|segment| segment.starts_with('.')) {
                return Err(Error::new(
                    ErrorCode::Unsupported,
                    "Native daily documents require a visible Markdown path",
                )
                .into());
            }
            guards::write(&state, &path, None, false)?;
            if state.root.stat(&path)?.is_some() {
                return Err(Error::new(
                    ErrorCode::Conflict,
                    "Daily document target changed before creation",
                )
                .into());
            }
            let (parent, _) = path.rsplit_once('/').ok_or_else(|| {
                Error::new(
                    ErrorCode::InvalidRequest,
                    "Default daily documents require a containing folder",
                )
            })?;
            parents(&state, parent)?;
            let version =
                operations::write(&state, &path, &text, &ExpectedVersion::Missing {}, None)?;
            Ok(Some(TextSnapshot {
                path,
                text,
                version,
            }))
        }
    }
}
impl Work {
    /// Native-only default policy, without selecting a host or provider owner.
    pub fn locate_default_daily_document(
        &self,
        date: &str,
    ) -> DailyDocumentOperationResult<Option<TextSnapshot>> {
        finish(self, prepare(self, date, None)?)
    }
    pub fn ensure_default_daily_document(
        &self,
        date: &str,
        settings: &DailyDocumentSettings,
        context: &DailyFormatContext,
    ) -> DailyDocumentOperationResult<TextSnapshot> {
        finish(self, prepare(self, date, Some((settings, context)))?)?.ok_or_else(|| {
            Error::new(
                ErrorCode::Io,
                "Default daily provider did not resolve a document",
            )
            .into()
        })
    }
}
