use super::*;
use std::{sync::Weak, time::Duration};

pub(super) fn run(vault: Weak<VaultService>, owner: Weak<Owner>) {
    loop {
        let Some(owner) = owner.upgrade() else {
            return;
        };
        if owner.stopped.load(Ordering::Acquire) {
            return;
        }
        let Some(vault) = vault.upgrade() else {
            return;
        };
        let result = cycle(&vault, &owner);
        drop(vault);
        if result.is_err() {
            // A failed journal publication fences the owner until cold inspection.
            owner.stop();
            return;
        }
        let Ok(wake) = owner.wake.lock() else {
            return;
        };
        let _ = owner.changed.wait_timeout(wake, Duration::from_secs(1));
    }
}

fn cycle(vault: &Arc<VaultService>, owner: &Arc<Owner>) -> Result<()> {
    if vault.lock()?.phase != lapis_app_protocol::VaultPhase::Open {
        return Ok(());
    }
    let mut stored = owner.store()?;
    if !stored.catalog.trusted || stored.catalog.safe_mode {
        return Ok(());
    }
    let mut selected = None;
    for (index, job) in stored.catalog.jobs.iter().enumerate() {
        if !job.configuration.enabled
            || job.transferred_to.is_some()
            || job
                .runs
                .iter()
                .any(|run| matches!(run.phase, JobPhase::Running | JobPhase::Interrupted))
        {
            continue;
        }
        let occurrence = job
            .configuration
            .schedule
            .latest(now())
            .map_err(|message| invalid(&message))?;
        if job.requested || occurrence.scheduled_at > job.last_scheduled_at {
            let source = match discovery::source(vault, job) {
                Ok(source) => source,
                Err(_) => continue,
            };
            selected = Some((index, occurrence, source));
            break;
        }
    }
    let Some((index, occurrence, source)) = selected else {
        return Ok(());
    };
    let work = match vault.begin_work() {
        Ok(work) => work,
        Err(_) => return Ok(()),
    };
    let run = JobRun {
        id: Uuid::new_v4().to_string(),
        occurrence: occurrence.key.clone(),
        local_date: occurrence.local_date,
        scheduled_at: occurrence.scheduled_at,
        started_at: now(),
        finished_at: None,
        phase: JobPhase::Running,
        error: None,
    };
    stored.update(|catalog| {
        let job = &mut catalog.jobs[index];
        job.requested = false;
        job.last_occurrence = Some(occurrence.key);
        job.last_scheduled_at = job.last_scheduled_at.max(occurrence.scheduled_at);
        if job.runs.len() == 16 {
            job.runs.remove(0);
        }
        job.runs.push(run.clone());
        Ok(())
    })?;
    let plugin = stored.catalog.jobs[index].plugin.clone();
    let job = stored.catalog.jobs[index].descriptor.id.clone();
    let context = serde_json::json!({"runId":run.id, "scheduledAt":run.scheduled_at, "localDate":run.local_date,
        "occurrence": run.occurrence, "pluginId":plugin, "jobId":job});
    owner.cancelled.store(false, Ordering::Release);
    let broker = Arc::new(broker::Broker {
        owner: owner.clone(),
        work,
        index,
        uncertain: AtomicBool::new(false),
        last_error: Mutex::new(None),
    });
    drop(stored);
    let result = execute(
        &source,
        &context,
        broker.clone(),
        ExecutionLimits::default(),
    );
    let uncertain = broker.uncertain.load(Ordering::Acquire);
    let failure = result.as_ref().err().map(|error| {
        broker
            .last_error
            .lock()
            .ok()
            .and_then(|error| error.clone())
            .unwrap_or_else(|| error.clone())
            .chars()
            .take(512)
            .collect::<String>()
    });
    // The counted Work outlives both handler execution and final journal publication.
    owner.store()?.update(|catalog| {
        let job = &mut catalog.jobs[index];
        let run = job
            .runs
            .last_mut()
            .ok_or_else(|| invalid("Background run disappeared"))?;
        run.finished_at = Some(now());
        run.phase = if uncertain {
            JobPhase::Interrupted
        } else if result.is_ok() {
            JobPhase::Completed
        } else {
            JobPhase::Failed
        };
        run.error = failure;
        if uncertain {
            job.configuration.enabled = false;
        }
        Ok(())
    })?;
    drop(broker);
    Ok(())
}
