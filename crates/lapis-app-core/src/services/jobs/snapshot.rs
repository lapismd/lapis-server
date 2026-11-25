use super::*;

pub(super) fn snapshot(
    vault: &VaultService,
    owner: &Owner,
    transfer: Option<JobTransfer>,
) -> Result<JobSnapshot> {
    let installed = discovery::discover(vault)?;
    let stored = owner.store()?;
    let mut jobs = Vec::new();
    for entry in installed {
        let saved = stored
            .catalog
            .jobs
            .iter()
            .find(|job| job.plugin == entry.plugin && job.descriptor.id == entry.descriptor.id);
        let unavailable_reason = saved
            .filter(|job| job.configuration.fingerprint != entry.fingerprint)
            .map(|_| "Plugin changed; review its background permissions again".into());
        let enabled = saved.is_some_and(|job| job.configuration.enabled)
            && stored.catalog.trusted
            && !stored.catalog.safe_mode
            && unavailable_reason.is_none();
        let phase = if let Some(job) = saved {
            if job.transferred_to.is_some() {
                JobPhase::Transferred
            } else if let Some(run) = job
                .runs
                .last()
                .filter(|run| matches!(run.phase, JobPhase::Running | JobPhase::Interrupted))
            {
                run.phase
            } else if enabled {
                JobPhase::Ready
            } else {
                JobPhase::Paused
            }
        } else {
            JobPhase::Paused
        };
        let next_run_at = if enabled {
            saved.and_then(|job| {
                job.configuration
                    .schedule
                    .next(now().max(job.last_scheduled_at))
                    .ok()
            })
        } else {
            None
        };
        jobs.push(JobStatus {
            plugin_id: entry.plugin,
            descriptor: entry.descriptor,
            fingerprint: entry.fingerprint,
            configuration: saved.map(|job| job.configuration.clone()),
            phase,
            runs: saved.map(|job| job.runs.clone()).unwrap_or_default(),
            next_run_at,
            unavailable_reason,
        });
    }
    // Disabled/uninstalled declarations retain recovery history and owner controls.
    for job in &stored.catalog.jobs {
        if jobs
            .iter()
            .any(|entry| entry.plugin_id == job.plugin && entry.descriptor.id == job.descriptor.id)
        {
            continue;
        }
        jobs.push(JobStatus {
            plugin_id: job.plugin.clone(),
            descriptor: job.descriptor.clone(),
            fingerprint: job.configuration.fingerprint.clone(),
            configuration: Some(job.configuration.clone()),
            phase: JobPhase::Paused,
            runs: job.runs.clone(),
            next_run_at: None,
            unavailable_reason: Some("Plugin is disabled, revoked or unavailable".into()),
        });
    }
    Ok(JobSnapshot {
        revision: stored.catalog.revision,
        runner_id: stored.catalog.runner.clone(),
        vault_id: vault.id.to_string(),
        native_notifications_available: owner.notification_delivery.is_some(),
        trusted: stored.catalog.trusted,
        safe_mode: stored.catalog.safe_mode,
        jobs,
        transfer,
    })
}
