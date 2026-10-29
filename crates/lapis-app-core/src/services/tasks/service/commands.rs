use super::{Data, errors, owner, projection, storage, unavailable, unselected};
use crate::services::{
    Error, Result, TaskLifecyclePolicy, TaskLifecycleRequest, TrackedTaskProvider, VaultService,
    Work, daily::DailyDocumentError,
};
use lapis_app_protocol::{ExpectedVersion, native_tasks::*};
use std::sync::Arc;

fn status(vault: &VaultService, configuration: Option<NativeTaskConfiguration>) -> NativeTaskValue {
    let tracked = vault
        .indexing_status()
        .ok()
        .and_then(|status| status.tracked)
        .map(projection::tracked);
    NativeTaskValue::Status {
        status: Box::new(NativeTasksStatus {
            enabled: configuration.is_some(),
            configuration,
            tracked,
        }),
    }
}
fn configured(data: &Data) -> Result<(&NativeTaskConfiguration, TaskLifecyclePolicy)> {
    let configuration = data.configuration.as_ref().ok_or_else(unselected)?;
    Ok((configuration, projection::policy(configuration)?))
}
fn lifecycle(data: &mut Data, work: Work, request: NativeTaskLifecycleRequest) -> NativeTaskValue {
    let id = request.request_id;
    let result = (|| {
        let (configuration, policy) =
            configured(data).map_err(crate::services::TaskRecoveryError::from)?;
        let request: TaskLifecycleRequest =
            projection::input(request).map_err(crate::services::TaskRecoveryError::from)?;
        // Duplicate acceptance must precede fresh source/provider preflight.
        if data.journal.inspect(&work, id)?.is_none()
            && let NativeTaskProvider::Unavailable { message } = &configuration.provider
        {
            let original = super::super::lifecycle::original(
                &work,
                &request.path,
                &ExpectedVersion::Version {
                    version: request.expected_version.clone(),
                },
            )?;
            let plan = lapis_task_services::plan_task_lifecycle(
                &original.text,
                &request.path,
                &request.action,
                &policy.day_parts,
            )
            .map_err(crate::services::TaskWriteError::from)?;
            if plan.observation.is_some() {
                return Err(crate::services::daily::DailyDocumentOperationError::Policy(
                    DailyDocumentError::new(message.clone()),
                )
                .into());
            }
        }
        data.journal.submit(work, request, &policy)
    })();
    match result {
        Ok(value) => match projection::operation(value) {
            Ok(value) => NativeTaskValue::Operation {
                operation: Some(Box::new(value)),
            },
            Err(error) => errors::value(errors::service(error)),
        },
        Err(error) => errors::value(errors::recovery(error, id)),
    }
}
pub(super) fn execute(
    vault: Arc<VaultService>,
    work: Work,
    command: NativeTaskCommand,
    runtime: tokio::runtime::Handle,
) -> Result<NativeTaskValue> {
    storage(&vault)?;
    if matches!(command, NativeTaskCommand::Status {}) {
        let slot = vault.tasks.lock().map_err(|_| unavailable())?.clone();
        return Ok(match slot {
            Some(owner) => status(
                &vault,
                owner
                    .data
                    .lock()
                    .map_err(|_| unavailable())?
                    .configuration
                    .clone(),
            ),
            None => status(&vault, None),
        });
    }
    let owner = owner(&vault)?;
    let mut data = owner.data.lock().map_err(|_| unavailable())?;
    match command {
        NativeTaskCommand::Configure { configuration } => {
            let policy = projection::policy(&configuration)?;
            let provider = match &configuration.provider {
                NativeTaskProvider::Default {} => TrackedTaskProvider::Default(policy),
                NativeTaskProvider::Unavailable { message } => {
                    TrackedTaskProvider::Unavailable(DailyDocumentError::new(message.clone()))
                }
            };
            runtime.block_on(vault.set_tracked_task_materialization(Some(provider)))?;
            data.configuration = Some(configuration);
            Ok(status(&vault, data.configuration.clone()))
        }
        NativeTaskCommand::Disable {} => {
            data.configuration = None;
            runtime.block_on(vault.set_tracked_task_materialization(None))?;
            Ok(status(&vault, None))
        }
        NativeTaskCommand::Mutate {
            path,
            expected_version,
            mutation,
        } => {
            configured(&data)?;
            let mutation = projection::input(mutation)?;
            let _lifecycle = vault.task_lifecycle.lock().map_err(|_| Error::busy())?;
            Ok(
                match work.mutate_task(
                    &path,
                    &ExpectedVersion::Version {
                        version: expected_version,
                    },
                    &mutation,
                ) {
                    Ok(snapshot) => NativeTaskValue::Snapshot { snapshot },
                    Err(error) => errors::value(errors::write(error)),
                },
            )
        }
        NativeTaskCommand::Submit { request } => Ok(lifecycle(&mut data, work, request)),
        NativeTaskCommand::Inspect { request_id } => {
            Ok(match data.journal.inspect(&work, request_id) {
                Ok(value) => NativeTaskValue::Operation {
                    operation: value.map(projection::operation).transpose()?.map(Box::new),
                },
                Err(error) => errors::value(errors::recovery(error, request_id)),
            })
        }
        NativeTaskCommand::Retry { request_id } => {
            configured(&data)?;
            Ok(match data.journal.retry(work, request_id) {
                Ok(value) => NativeTaskValue::Operation {
                    operation: Some(Box::new(projection::operation(value)?)),
                },
                Err(error) => errors::value(errors::recovery(error, request_id)),
            })
        }
        NativeTaskCommand::Occurrence {
            path,
            expected_version,
            date,
            completion,
        } => {
            let (configuration, policy) = configured(&data)?;
            if let NativeTaskProvider::Unavailable { message } = &configuration.provider {
                return Ok(errors::value(errors::provider(message.clone())));
            }
            Ok(super::observations::apply(
                &work,
                &path,
                &expected_version,
                &date,
                completion,
                &policy,
            ))
        }
        NativeTaskCommand::Status {} => unreachable!(),
    }
}
