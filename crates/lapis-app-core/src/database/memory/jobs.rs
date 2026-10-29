use super::{client::valid_job, enum_text, json, text};
use crate::database::{engine, store::Store};
use crate::services::Result;
use lapis_app_protocol::database::memory::{enums::MemoryJobStatus as Status, jobs::*};

impl Store {
    async fn memory_job(&self, id: &str) -> Result<Option<MemoryJob>> {
        self.memory_rows(
            "SELECT data_json FROM memory_jobs WHERE id=?",
            vec![text(id)],
        )
        .await
        .map(|mut r| r.pop())
    }
    pub(super) async fn claim_memory(&self, input: MemoryJobClaim) -> Result<Option<MemoryJob>> {
        let existing = self.memory_job(&input.job.id).await?;
        if existing
            .as_ref()
            .is_some_and(|j| j.status == Status::Completed)
        {
            return Ok(None);
        }
        let renewing = existing.as_ref().is_some_and(|j| {
            j.status == Status::Running && j.owner_id.as_deref() == Some(&input.owner_id)
        });
        if existing.as_ref().is_some_and(|j| {
            j.status == Status::Running
                && j.owner_id.as_deref() != Some(&input.owner_id)
                && j.lease_until.unwrap_or(0.) > input.now
        }) {
            return Ok(None);
        }
        let mut job = existing.unwrap_or(input.job);
        if !renewing {
            job.attempts += 1.;
        }
        if job.attempts > job.max_attempts {
            return Ok(None);
        }
        job.status = Status::Running;
        job.owner_id = Some(input.owner_id);
        job.lease_until = Some(input.now + input.lease_ms.max(1.));
        if !renewing {
            job.started_at = Some(input.now);
        }
        job.finished_at = None;
        job.error_code = None;
        self.write_memory_job(&job).await?;
        Ok(Some(job))
    }
    pub(super) async fn update_memory(&self, input: MemoryJobUpdate) -> Result<Option<MemoryJob>> {
        let Some(mut job) = self.memory_job(&input.job_id).await? else {
            return Ok(None);
        };
        if job.status != Status::Running
            || job.owner_id.as_deref() != Some(&input.owner_id)
            || job.lease_until.unwrap_or(0.) < input.now
        {
            return Ok(None);
        }
        job.status = input.patch.status;
        if let Some(cursor) = input.patch.cursor {
            job.cursor = Some(cursor);
        }
        if let Some(time) = input.patch.finished_at {
            job.finished_at = Some(time);
        }
        if let Some(error) = input.patch.error_code {
            job.error_code = Some(error);
        }
        if job.status == Status::Running {
            job.lease_until = input.patch.lease_until.or(job.lease_until);
        } else {
            job.owner_id = None;
            job.lease_until = None;
        }
        self.write_memory_job(&job).await?;
        Ok(Some(job))
    }
    async fn write_memory_job(&self, job: &MemoryJob) -> Result<()> {
        valid_job(job)?;
        self.connection.execute("INSERT INTO memory_jobs(id,scope_key,status,lease_until,created_at,data_json) VALUES(?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET scope_key=excluded.scope_key,status=excluded.status,lease_until=excluded.lease_until,created_at=excluded.created_at,data_json=excluded.data_json",turso::params![job.id.as_str(),job.scope_key.as_str(),enum_text(&job.status)?,job.lease_until,job.created_at,json(job)?]).await.map_err(engine)?;
        Ok(())
    }
}
