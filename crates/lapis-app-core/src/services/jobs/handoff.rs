use super::control::{active, find, permissions};
use super::*;

pub(super) fn release(
    owner: &Owner,
    plugin: &str,
    id: &str,
    destination: &str,
    destination_vault: &str,
) -> Result<JobTransfer> {
    Uuid::parse_str(destination).map_err(|_| invalid("Invalid destination runner identity"))?;
    Uuid::parse_str(destination_vault)
        .map_err(|_| invalid("Invalid destination vault identity"))?;
    let mut receipt = None;
    owner.store()?.update(|catalog| {
        if destination == catalog.runner {
            return Err(invalid("Select a different runner"));
        }
        let from_runner = catalog.runner.clone();
        let vault_id = catalog.vault.to_string();
        let job = find(catalog, plugin, id)?;
        if let Some(existing) = &job.release {
            if existing.to_runner != destination || existing.to_vault_id != destination_vault {
                return Err(invalid("This job already has a designated destination"));
            }
            receipt = Some(existing.clone());
            return Ok(());
        }
        if active(job)
            || job
                .runs
                .iter()
                .any(|run| run.phase == JobPhase::Interrupted)
        {
            return Err(Error::busy());
        }
        job.configuration.enabled = false;
        job.requested = false;
        job.transferred_to = Some(destination.into());
        let value = JobTransfer {
            id: Uuid::new_v4().to_string(),
            from_vault_id: vault_id,
            to_vault_id: destination_vault.into(),
            from_runner,
            to_runner: destination.into(),
            plugin_id: plugin.into(),
            job_id: id.into(),
            configuration: job.configuration.clone(),
            state: job.state.clone(),
            last_occurrence: job.last_occurrence.clone(),
            last_scheduled_at: job.last_scheduled_at,
            notification_keys: job.notifications.clone(),
        };
        job.release = Some(value.clone());
        receipt = Some(value);
        Ok(())
    })?;
    receipt.ok_or_else(|| invalid("Job handoff was not persisted"))
}

pub(super) fn accept(vault: &VaultService, owner: &Owner, mut receipt: JobTransfer) -> Result<()> {
    for id in [
        &receipt.id,
        &receipt.to_runner,
        &receipt.from_runner,
        &receipt.to_vault_id,
        &receipt.from_vault_id,
    ] {
        Uuid::parse_str(id).map_err(|_| invalid("Invalid handoff identity"))?;
    }
    if receipt.to_vault_id != vault.id.to_string()
        || receipt.to_runner != owner.store()?.catalog.runner
        || receipt.to_runner == receipt.from_runner
    {
        return Err(Error::denied());
    }
    let installed = discovery::discover(vault)?
        .into_iter()
        .find(|job| job.plugin == receipt.plugin_id && job.descriptor.id == receipt.job_id)
        .ok_or_else(|| invalid("Install and enable the transferred plugin first"))?;
    if receipt.configuration.fingerprint != installed.fingerprint
        || !permissions(
            &receipt.configuration.permissions,
            &installed.descriptor.permissions,
        )
        || receipt.notification_keys.len() > 128
        || receipt.notification_keys.iter().any(|key| key.len() > 128)
        || serde_json::to_vec(&receipt.state)
            .map_err(|_| invalid("Invalid transfer state"))?
            .len()
            > 65536
    {
        return Err(Error::denied());
    }
    receipt
        .configuration
        .schedule
        .validate()
        .map_err(|message| invalid(&message))?;
    receipt.configuration.enabled = false;
    receipt.configuration.native_notifications = false;
    owner.store()?.update(|catalog| {
        if !catalog.trusted
            || catalog.safe_mode
            || catalog.imports.contains(&receipt.id)
            || catalog.imports.len() >= 256
        {
            return Err(Error::denied());
        }
        let existing = catalog
            .jobs
            .iter()
            .position(|job| job.plugin == receipt.plugin_id && job.descriptor.id == receipt.job_id);
        if let Some(index) = existing {
            let old = &catalog.jobs[index];
            if active(old)
                || old
                    .runs
                    .iter()
                    .any(|run| run.phase == JobPhase::Interrupted)
            {
                return Err(Error::busy());
            }
        } else if catalog.jobs.len() >= 64 {
            return Err(invalid("Background job limit reached"));
        }
        let job = storage::StoredJob {
            plugin: receipt.plugin_id,
            descriptor: installed.descriptor,
            configuration: receipt.configuration,
            state: receipt.state,
            last_occurrence: receipt.last_occurrence,
            last_scheduled_at: receipt.last_scheduled_at,
            runs: Vec::new(),
            requested: false,
            transferred_to: None,
            notifications: receipt.notification_keys,
            release: None,
        };
        if let Some(index) = existing {
            catalog.jobs[index] = job;
        } else {
            catalog.jobs.push(job);
        }
        catalog.imports.push(receipt.id);
        Ok(())
    })
}
