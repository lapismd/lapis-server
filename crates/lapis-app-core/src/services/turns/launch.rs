use super::{Error, Job, Owner, Work};
use crate::vault::VaultRoot;
use lapis_ai_service::{Launch, SessionBinding, runtime::SessionProcessor};
use lapis_app_protocol::{
    ErrorCode,
    turns::{TurnInput, TurnRecord},
};

pub(super) struct Selected {
    scope: VaultRoot,
    launch: Launch,
    pub history: Option<super::history::History>,
    context: Option<String>,
}

pub(super) struct Prepared {
    pub scope: VaultRoot,
    pub launch: Launch,
    pub binding: SessionBinding,
    pub processor: Option<Box<dyn SessionProcessor>>,
    pub context: Option<String>,
}
pub(super) fn select(owner: &Owner, work: &Work, input: &TurnInput) -> super::Result<Selected> {
    let scope = {
        let state = work.vault.lock()?;
        if input.scope_dir.is_empty() {
            state.root.try_clone()
        } else {
            state.root.open_child(&input.scope_dir)
        }
    }?;
    let launch = owner
        .storage
        .policy
        .resolve(work.vault.id, &scope, &input.target_id)
        .map_err(|_| Error::new(ErrorCode::Unsupported, "Native agent target is unavailable"))?;
    if launch.directory != scope.path()
        || launch.prepared.directory != scope.path()
        || !launch.prepared.arguments.is_empty()
        || launch.resume.is_some()
    {
        return Err(Error::new(
            ErrorCode::PermissionDenied,
            "Native launch scope is invalid",
        ));
    }
    scope.verify_physical_root()?;
    let history = super::history::read(work, input)?;
    let context = history
        .as_ref()
        .map(|history| match input.context.as_deref() {
            Some(context) if !context.is_empty() => format!("{}\n\n{context}", history.context),
            _ => history.context.clone(),
        })
        .or_else(|| input.context.clone());
    lapis_ai_service::runtime::SessionRequest::Prompt {
        text: input.prompt.clone(),
        context: context.clone(),
        max_duration_ms: input.options.max_duration_ms,
    }
    .validate(&input.request_id.to_string())
    .map_err(|_| {
        Error::new(
            ErrorCode::InvalidRequest,
            "Native conversation context exceeds runtime limits",
        )
    })?;
    Ok(Selected {
        scope,
        launch,
        history,
        context,
    })
}
pub(super) fn prepare(
    owner: &Owner,
    job: &Job,
    record: &TurnRecord,
    selected: Selected,
) -> Result<Prepared, &'static str> {
    let Selected {
        scope,
        launch,
        context,
        ..
    } = selected;
    scope
        .verify_physical_root()
        .map_err(|_| "native_scope_changed")?;
    let binding = SessionBinding {
        session_id: job.participant.as_ref().map_or_else(
            || uuid::Uuid::new_v4().to_string(),
            |p| p.session_id.to_string(),
        ),
        request_id: record.input.request_id.to_string(),
        target_id: record.input.target_id.clone(),
        admission: job.work.vault.generation.to_string(),
    };
    let processor = owner
        .storage
        .policy
        .processor(job.work.vault.id, &binding)
        .map_err(|e| e.0)?;
    Ok(Prepared {
        scope,
        launch,
        binding,
        processor,
        context,
    })
}
