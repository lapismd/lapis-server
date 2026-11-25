use lapis_app_core::services::{Client, JobStorage, Runtime, VaultService};
use lapis_app_protocol::background_jobs::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

struct Fixture {
    vault: tempfile::TempDir,
    profile: tempfile::TempDir,
    id: Uuid,
    runtime: Runtime,
    owner: Arc<VaultService>,
}
impl Fixture {
    fn new(source: &str) -> Self {
        Self::with_options(source, false, false, None)
    }
    fn with_options(
        source: &str,
        metadata: bool,
        notifications: bool,
        delivery: Option<lapis_app_core::services::JobNotificationDelivery>,
    ) -> Self {
        let vault = tempfile::tempdir().unwrap();
        let profile = tempfile::tempdir().unwrap();
        fs::create_dir_all(vault.path().join(".lapis/plugins/example")).unwrap();
        fs::write(
            vault.path().join("note.md"),
            "---\nstatus: original\nuntouched: 'quoted'\n---\nBody stays.\n",
        )
        .unwrap();
        let manifest = json!({"id":"example","version":"1.0.0","lapis":{"contributes":{"backgroundJobs":[{
            "id":"refresh","title":"Refresh","description":"Test handler","entry":"job.js",
            "schedule":{"kind":"interval","seconds":60},"permissions":{"metadataRead":metadata,"frontmatterRead":true,
            "notifications":notifications,"frontmatterFields":["status"]}}]}}}).to_string();
        let files: Vec<_> = [("job.js", source), ("manifest.json", manifest.as_str())].iter().map(|(path, text)| {
            fs::write(vault.path().join(".lapis/plugins/example").join(path), text).unwrap();
            json!({"path":path,"size":text.len(),"sha256":format!("{:x}",Sha256::digest(text.as_bytes()))})
        }).collect();
        fs::write(
            vault.path().join(".lapis/installed-plugins.json"),
            json!({"schemaVersion":1,"plugins":{"example":{
            "pluginId":"example","installedVersion":"1.0.0","files":files}}})
            .to_string(),
        )
        .unwrap();
        fs::write(
            vault.path().join(".lapis/community-plugins.json"),
            "[\"example\"]",
        )
        .unwrap();
        let id = Uuid::new_v4();
        fs::create_dir(profile.path().join("jobs")).unwrap();
        fs::create_dir(profile.path().join("database")).unwrap();
        let mut storage = JobStorage::new(&profile.path().join("jobs")).unwrap();
        if let Some(delivery) = delivery {
            storage = storage.with_notification_delivery(delivery);
        }
        let runtime = Runtime::with_database(
            lapis_app_core::services::DatabaseStorage::new(
                &profile.path().join("database"),
                lapis_app_core::embedding::host::NativeEmbeddingHost::default(),
            )
            .unwrap(),
        )
        .with_jobs(storage);
        let owner = runtime.admit(id, vault.path()).unwrap();
        Self {
            vault,
            profile,
            id,
            runtime,
            owner,
        }
    }
    fn client(&self) -> Client {
        self.owner.attach().unwrap()
    }
    fn configure(&self) -> JobSnapshot {
        let client = self.client();
        let snapshot = client.plugin_jobs(JobCommand::List {}).unwrap();
        let job = &snapshot.jobs[0];
        client
            .plugin_jobs(JobCommand::Configure {
                plugin_id: "example".into(),
                job_id: "refresh".into(),
                configuration: JobConfiguration {
                    fingerprint: job.fingerprint.clone(),
                    schedule: job.descriptor.schedule.clone(),
                    permissions: job.descriptor.permissions.clone(),
                    enabled: true,
                    native_notifications: false,
                },
            })
            .unwrap()
    }
    fn start(&self) {
        let client = self.client();
        let before = client.plugin_jobs(JobCommand::List {}).unwrap().jobs[0]
            .runs
            .len();
        client
            .plugin_jobs(JobCommand::RunNow {
                plugin_id: "example".into(),
                job_id: "refresh".into(),
            })
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if client.plugin_jobs(JobCommand::List {}).unwrap().jobs[0]
                .runs
                .len()
                > before
            {
                break;
            }
            assert!(Instant::now() < deadline, "Job was not admitted");
            thread::sleep(Duration::from_millis(10));
        }
    }
    fn wait(&self, phase: JobPhase) -> JobSnapshot {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let snapshot = self.client().plugin_jobs(JobCommand::List {}).unwrap();
            if snapshot.jobs[0]
                .runs
                .last()
                .is_some_and(|run| run.phase == phase)
            {
                return snapshot;
            }
            assert!(
                Instant::now() < deadline,
                "Job did not reach {phase:?}: {snapshot:?}"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }
    fn reopen(&mut self) {
        self.runtime.close_vault(self.id).unwrap();
        self.owner = self.runtime.admit(self.id, self.vault.path()).unwrap();
    }
}

const UPDATE: &str = "export function run(ctx) { const note=ctx.notes.frontmatter('note.md'); ctx.notes.patchFrontmatter({path:'note.md',expectedVersion:note.version,set:{status:'refreshed'}}); }";

#[test]
fn installed_jobs_are_opt_in_and_preserve_note_body_and_unrelated_yaml() {
    let fixture = Fixture::new(UPDATE);
    let before = fixture.client().plugin_jobs(JobCommand::List {}).unwrap();
    assert_eq!(before.jobs[0].phase, JobPhase::Paused);
    fixture.configure();
    fixture.start();
    fixture.wait(JobPhase::Completed);
    let text = fs::read_to_string(fixture.vault.path().join("note.md")).unwrap();
    assert_eq!(
        lapis_plugin_services::markdown::read_frontmatter(&text).unwrap()["status"],
        "refreshed"
    );
    assert!(text.contains("untouched: 'quoted'"));
    assert!(text.ends_with("Body stays.\n"));
    fixture.runtime.close_vault(fixture.id).unwrap();
}

#[test]
fn dirty_editors_fence_job_writes() {
    let fixture = Fixture::new(UPDATE);
    let client = fixture.client();
    let editor = client.open_editor("note.md").unwrap();
    client.mark_dirty(editor, 1).unwrap();
    fixture.configure();
    fixture.start();
    fixture.wait(JobPhase::Failed);
    assert!(
        fs::read_to_string(fixture.vault.path().join("note.md"))
            .unwrap()
            .contains("status: original")
    );
    client.discard_editor(editor).unwrap();
    fixture.runtime.close_vault(fixture.id).unwrap();
}

#[test]
fn native_jobs_continue_with_zero_attached_clients_and_retain_checkpoints() {
    let mut fixture = Fixture::new(
        "export function run(ctx) { const value=ctx.state.load()||0; ctx.state.save(value+1); }",
    );
    fixture.configure();
    fixture.start();
    assert_eq!(fixture.owner.status().unwrap().clients, 0);
    fixture.wait(JobPhase::Completed);
    fixture.reopen();
    fixture.start();
    let snapshot = fixture.wait(JobPhase::Completed);
    assert_eq!(snapshot.jobs[0].runs.len(), 2);
    let directory = fixture
        .profile
        .path()
        .join("jobs")
        .join(fixture.id.to_string());
    let record = fs::read_dir(directory)
        .unwrap()
        .filter_map(Result::ok)
        .find(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "json")
        })
        .unwrap();
    let stored: Value = serde_json::from_slice(&fs::read(record.path()).unwrap()).unwrap();
    assert_eq!(stored["jobs"][0]["state"], 2);
    fixture.runtime.close_vault(fixture.id).unwrap();
}

#[test]
fn pause_interrupts_cpu_work_and_close_drains_the_native_owner() {
    let fixture = Fixture::new("export function run() { while(true) {} }");
    fixture.configure();
    fixture.start();
    fixture.wait(JobPhase::Running);
    fixture
        .client()
        .plugin_jobs(JobCommand::Pause {
            plugin_id: "example".into(),
            job_id: "refresh".into(),
        })
        .unwrap();
    fixture.wait(JobPhase::Failed);
    fixture.configure();
    fixture.start();
    fixture.wait(JobPhase::Running);
    let start = Instant::now();
    fixture.runtime.close_vault(fixture.id).unwrap();
    assert!(start.elapsed() < Duration::from_secs(3));
}

#[test]
fn changed_installed_bytes_never_execute() {
    let fixture = Fixture::new(UPDATE);
    fixture.configure();
    fs::write(
        fixture.vault.path().join(".lapis/plugins/example/job.js"),
        "export function run() { throw Error('changed'); }",
    )
    .unwrap();
    assert!(
        fixture
            .client()
            .plugin_jobs(JobCommand::RunNow {
                plugin_id: "example".into(),
                job_id: "refresh".into()
            })
            .is_err()
    );
    thread::sleep(Duration::from_millis(1200));
    let snapshot = fixture.client().plugin_jobs(JobCommand::List {}).unwrap();
    assert!(snapshot.jobs[0].runs.is_empty());
    assert!(
        fs::read_to_string(fixture.vault.path().join("note.md"))
            .unwrap()
            .contains("status: original")
    );
    fixture.runtime.close_vault(fixture.id).unwrap();
}

#[test]
fn trust_and_safe_mode_fence_manual_admission() {
    let fixture = Fixture::new(UPDATE);
    fixture.configure();
    let client = fixture.client();
    for (trusted, safe_mode) in [(false, false), (true, true)] {
        client
            .plugin_jobs(JobCommand::Fence { trusted, safe_mode })
            .unwrap();
        assert!(
            client
                .plugin_jobs(JobCommand::RunNow {
                    plugin_id: "example".into(),
                    job_id: "refresh".into()
                })
                .is_err()
        );
    }
    client
        .plugin_jobs(JobCommand::Fence {
            trusted: true,
            safe_mode: false,
        })
        .unwrap();
    fixture.start();
    fixture.wait(JobPhase::Completed);
    fixture.runtime.close_vault(fixture.id).unwrap();
}

#[test]
fn handoff_persists_release_before_destination_import_and_rejects_replay() {
    let fixture = Fixture::new("export function run(ctx) { ctx.state.save(17); }");
    fixture.configure();
    fixture.start();
    fixture.wait(JobPhase::Completed);
    let destination = tempfile::tempdir().unwrap();
    let runtime = Runtime::default().with_jobs(JobStorage::new(destination.path()).unwrap());
    let destination_vault = Uuid::new_v4();
    let owner = runtime
        .admit(destination_vault, fixture.vault.path())
        .unwrap();
    let target = owner.attach().unwrap();
    let target_id = target.plugin_jobs(JobCommand::List {}).unwrap().runner_id;
    let released = fixture
        .client()
        .plugin_jobs(JobCommand::Transfer {
            plugin_id: "example".into(),
            job_id: "refresh".into(),
            to_runner: target_id,
            to_vault_id: destination_vault.to_string(),
        })
        .unwrap();
    assert_eq!(released.jobs[0].phase, JobPhase::Transferred);
    let receipt = released.transfer.unwrap();
    assert_eq!(receipt.state, 17);
    let imported = target
        .plugin_jobs(JobCommand::ImportTransfer {
            receipt: Box::new(receipt.clone()),
        })
        .unwrap();
    assert!(!imported.jobs[0].configuration.as_ref().unwrap().enabled);
    assert!(
        target
            .plugin_jobs(JobCommand::ImportTransfer {
                receipt: Box::new(receipt)
            })
            .is_err()
    );
    runtime.close_vault(destination_vault).unwrap();
    fixture.runtime.close_vault(fixture.id).unwrap();
}

fn catalog_path(fixture: &Fixture) -> std::path::PathBuf {
    fs::read_dir(
        fixture
            .profile
            .path()
            .join("jobs")
            .join(fixture.id.to_string()),
    )
    .unwrap()
    .filter_map(Result::ok)
    .find(|entry| {
        entry
            .path()
            .extension()
            .is_some_and(|extension| extension == "json")
    })
    .unwrap()
    .path()
}

#[test]
fn cold_running_receipts_require_inspection_and_eligible_vaults_resume_from_private_storage() {
    let mut fixture = Fixture::new("export function run(ctx) { ctx.state.save(9); }");
    fixture.configure();
    assert_eq!(
        fixture.runtime.background_vault_ids().unwrap(),
        [fixture.id]
    );
    assert!(fixture.owner.has_background_jobs());
    fixture.start();
    fixture.wait(JobPhase::Completed);
    fixture.runtime.close_vault(fixture.id).unwrap();
    let path = catalog_path(&fixture);
    let mut catalog: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    catalog["jobs"][0]["runs"][0]["phase"] = json!("running");
    fs::write(&path, serde_json::to_vec(&catalog).unwrap()).unwrap();
    fixture.owner = fixture
        .runtime
        .admit(fixture.id, fixture.vault.path())
        .unwrap();
    let snapshot = fixture.client().plugin_jobs(JobCommand::List {}).unwrap();
    assert_eq!(snapshot.jobs[0].runs[0].phase, JobPhase::Interrupted);
    assert!(!snapshot.jobs[0].configuration.as_ref().unwrap().enabled);
    assert!(!fixture.owner.has_background_jobs());
    assert!(fixture.runtime.background_vault_ids().unwrap().is_empty());
    assert!(
        fixture
            .client()
            .plugin_jobs(JobCommand::RunNow {
                plugin_id: "example".into(),
                job_id: "refresh".into()
            })
            .is_err()
    );
    fixture.runtime.close_vault(fixture.id).unwrap();
}

#[test]
fn metadata_and_persistent_notifications_share_native_storage_and_deduplicate_os_delivery() {
    let delivered = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let sink = delivered.clone();
    let fixture = Fixture::with_options(
        "export function run(ctx) { const page=ctx.metadata.query({include:['properties'],limit:100}); if(!page.rows.some(row=>row.file.path==='note.md')) throw Error('Missing native metadata'); ctx.notifications.notify({id:'daily',message:'Saved vault digest'}); }",
        true,
        true,
        Some(Arc::new(move |_| {
            sink.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        })),
    );
    let mut snapshot = fixture.configure();
    let mut configuration = snapshot.jobs[0].configuration.take().unwrap();
    configuration.native_notifications = true;
    fixture
        .client()
        .plugin_jobs(JobCommand::Configure {
            plugin_id: "example".into(),
            job_id: "refresh".into(),
            configuration,
        })
        .unwrap();
    fixture.start();
    fixture.wait(JobPhase::Completed);
    fixture.start();
    fixture.wait(JobPhase::Completed);
    assert_eq!(delivered.load(std::sync::atomic::Ordering::SeqCst), 1);
    let notifications = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(fixture.client().call_database(
            lapis_app_protocol::database::call::DatabaseCall {
                method: lapis_app_protocol::database::call::DatabaseMethod::ListNotifications,
                args: vec![],
            },
        ))
        .unwrap();
    let lapis_app_protocol::database::call::DatabaseValue::Value { value } = notifications else {
        panic!("Notification history missing");
    };
    assert_eq!(value.as_array().unwrap().len(), 1);
    assert_eq!(value[0]["message"], "Saved vault digest");
    fixture.runtime.close_vault(fixture.id).unwrap();
}

#[test]
fn reapproval_cannot_rewind_a_saved_schedule_high_water() {
    let mut fixture = Fixture::new("export function run() {}");
    fixture.configure();
    fixture.runtime.close_vault(fixture.id).unwrap();
    let path = catalog_path(&fixture);
    let mut catalog: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let future = 4_102_444_800_000_i64;
    catalog["jobs"][0]["last_scheduled_at"] = json!(future);
    fs::write(&path, serde_json::to_vec(&catalog).unwrap()).unwrap();
    fixture.owner = fixture
        .runtime
        .admit(fixture.id, fixture.vault.path())
        .unwrap();
    let snapshot = fixture.configure();
    assert!(snapshot.jobs[0].next_run_at.unwrap() > future);
    let stored: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert_eq!(stored["jobs"][0]["last_scheduled_at"], future);
    fixture.runtime.close_vault(fixture.id).unwrap();
}
