use lapis_app_core::{
    credentials::{CredentialError, ScopedCredentialStore, SecretBytes, SecretName},
    services::{Client, JobStorage, Runtime},
};
use lapis_plugin_jobs::*;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs,
    io::{Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;

#[derive(Default)]
struct Secrets(Mutex<HashMap<String, Vec<u8>>>);
impl ScopedCredentialStore for Secrets {
    fn get(&self, name: &SecretName) -> Result<Option<SecretBytes>, CredentialError> {
        self.0
            .lock()
            .unwrap()
            .get(name.as_str())
            .cloned()
            .map(SecretBytes::new)
            .transpose()
    }
    fn set(&self, name: &SecretName, value: &SecretBytes) -> Result<(), CredentialError> {
        self.0
            .lock()
            .unwrap()
            .insert(name.as_str().into(), value.expose_secret().to_vec());
        Ok(())
    }
    fn delete(&self, name: &SecretName) -> Result<(), CredentialError> {
        self.0.lock().unwrap().remove(name.as_str());
        Ok(())
    }
}

struct Fixture {
    vault: tempfile::TempDir,
    profile: tempfile::TempDir,
    runtime: Runtime,
    client: Client,
    id: Uuid,
}
impl Fixture {
    fn new(source: &str, origin: &str, private: bool) -> Self {
        let vault = tempfile::tempdir().unwrap();
        let profile = tempfile::tempdir().unwrap();
        fs::create_dir_all(vault.path().join(".lapis/plugins/example")).unwrap();
        fs::create_dir(profile.path().join("jobs")).unwrap();
        let manifest = json!({"id":"example","version":"1.0.0","lapis":{"contributes":{"backgroundJobs":[{
            "id":"sync","title":"Bank sync","description":"Synthetic bank","entry":"job.js","schedule":{"kind":"interval","seconds":60},
            "permissions":{"metadataRead":false,"frontmatterRead":false,"notifications":false,"frontmatterFields":[],
                "network":[{"origin":origin,"allowPrivateNetwork":private,"methods":["GET","POST"],"credentials":[{"id":"bank","header":"authorization","prefix":"Bearer "}]}]}}]}}}).to_string();
        let files: Vec<_> = [("manifest.json", manifest.as_str()), ("job.js", source)].into_iter().map(|(path,text)| {
            fs::write(vault.path().join(".lapis/plugins/example").join(path),text).unwrap();
            json!({"path":path,"size":text.len(),"sha256":format!("{:x}",Sha256::digest(text.as_bytes()))})
        }).collect();
        fs::write(vault.path().join(".lapis/installed-plugins.json"), json!({"schemaVersion":1,"plugins":{"example":{"pluginId":"example","installedVersion":"1.0.0","files":files}}}).to_string()).unwrap();
        fs::write(
            vault.path().join(".lapis/community-plugins.json"),
            "[\"example\"]",
        )
        .unwrap();
        let scopes = Arc::new(Mutex::new(
            HashMap::<(Uuid, String, String), Arc<Secrets>>::new(),
        ));
        let storage = JobStorage::new(&profile.path().join("jobs"))
            .unwrap()
            .with_credentials(Arc::new(move |vault, plugin, job| {
                if plugin != "example" {
                    return Err(CredentialError::Unavailable);
                }
                let store = scopes
                    .lock()
                    .unwrap()
                    .entry((vault, plugin.into(), job.into()))
                    .or_default()
                    .clone();
                Ok(store as Arc<dyn ScopedCredentialStore>)
            }));
        let runtime = Runtime::default().with_jobs(storage);
        let id = Uuid::new_v4();
        let client = runtime.admit(id, vault.path()).unwrap().attach().unwrap();
        Self {
            vault,
            profile,
            runtime,
            client,
            id,
        }
    }
    fn snapshot(&self) -> JobSnapshot {
        self.client.plugin_jobs(JobCommand::List {}).unwrap()
    }
    fn configure(&self) {
        let job = self.snapshot().jobs.remove(0);
        self.client
            .plugin_jobs(JobCommand::Configure {
                plugin_id: "example".into(),
                job_id: "sync".into(),
                configuration: JobConfiguration {
                    fingerprint: job.fingerprint,
                    schedule: job.descriptor.schedule,
                    permissions: job.descriptor.permissions,
                    enabled: true,
                    native_notifications: false,
                },
            })
            .unwrap();
    }
    fn secret(&self, value: Option<&str>) {
        self.client
            .plugin_jobs(JobCommand::StoreCredential {
                plugin_id: "example".into(),
                job_id: "sync".into(),
                credential_id: "bank".into(),
                value: value.map(str::to_owned),
            })
            .unwrap();
    }
    fn start(&self) {
        self.client
            .plugin_jobs(JobCommand::RunNow {
                plugin_id: "example".into(),
                job_id: "sync".into(),
            })
            .unwrap();
    }
    fn wait(&self, phase: JobPhase) -> JobSnapshot {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let snapshot = self.snapshot();
            if snapshot.jobs[0]
                .runs
                .last()
                .is_some_and(|run| run.phase == phase)
            {
                return snapshot;
            }
            assert!(
                Instant::now() < deadline,
                "Expected {phase:?}, got {snapshot:?}"
            );
            thread::sleep(Duration::from_millis(20));
        }
    }
    fn journal(&self) -> String {
        let directory = self.profile.path().join("jobs").join(self.id.to_string());
        let path = fs::read_dir(directory)
            .unwrap()
            .map(Result::unwrap)
            .find(|e| e.path().extension().is_some_and(|e| e == "json"))
            .unwrap()
            .path();
        fs::read_to_string(path).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.runtime.close_vault(self.id).unwrap();
    }
}

fn server(reply: &'static str, stall: bool) -> (String, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut input = Vec::new();
        loop {
            let mut b = [0; 1];
            stream.read_exact(&mut b).unwrap();
            input.extend_from_slice(&b);
            if input.ends_with(b"\r\n\r\n") {
                break;
            }
        }
        tx.send(String::from_utf8(input).unwrap()).unwrap();
        stream.write_all(reply.as_bytes()).unwrap();
        if stall {
            thread::sleep(Duration::from_secs(4));
        }
    });
    (origin, rx)
}

#[test]
fn synthetic_bank_uses_opaque_native_credentials_and_keeps_secrets_out_of_job_records() {
    let (origin, request) = server(
        "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nContent-Type: application/json\r\nSet-Cookie: ignored=secret\r\n\r\n[]",
        false,
    );
    let source = format!(
        "export function run(ctx) {{ if(ctx.credentials||typeof fetch!=='undefined') throw Error('ambient'); const r=ctx.network.request({{url:'{origin}/transactions',credential:'bank'}}); if(r.headers['set-cookie']) throw Error('cookie'); ctx.state.save({{transactions:JSON.parse(r.body),status:r.status}}); }}"
    );
    let fixture = Fixture::new(&source, &origin, true);
    assert!(!fixture.snapshot().jobs[0].credentials.as_ref().unwrap()[0].configured);
    fixture.secret(Some("synthetic-bank-key"));
    assert!(fixture.snapshot().jobs[0].credentials.as_ref().unwrap()[0].configured);
    fixture.configure();
    fixture.start();
    fixture.wait(JobPhase::Completed);
    assert!(
        request
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .contains("authorization: Bearer synthetic-bank-key\r\n")
    );
    let journal = fixture.journal();
    assert!(!journal.contains("synthetic-bank-key"));
    let record: Value = serde_json::from_str(&journal).unwrap();
    assert_eq!(
        record["jobs"][0]["state"],
        json!({"transactions":[],"status":200})
    );
    fixture.secret(Some("rotated-bank-key"));
    fixture.secret(None);
    assert!(!fixture.snapshot().jobs[0].credentials.as_ref().unwrap()[0].configured);
    assert!(!fixture.journal().contains("rotated-bank-key"));
}

#[test]
fn pause_cancels_a_stalled_body_without_waiting_for_the_catalog_lock() {
    let (origin, request) = server("HTTP/1.1 200 OK\r\nContent-Length: 20\r\n\r\n", true);
    let source = format!(
        "export function run(ctx) {{ ctx.network.request({{url:'{origin}/transactions'}}); }}"
    );
    let fixture = Fixture::new(&source, &origin, true);
    fixture.configure();
    fixture.start();
    request.recv_timeout(Duration::from_secs(4)).unwrap();
    let start = Instant::now();
    fixture
        .client
        .plugin_jobs(JobCommand::Pause {
            plugin_id: "example".into(),
            job_id: "sync".into(),
        })
        .unwrap();
    fixture.wait(JobPhase::Failed);
    assert!(start.elapsed() < Duration::from_secs(2));
}

#[test]
fn caught_unknown_external_mutation_never_clears_interruption_on_a_later_checkpoint() {
    let (origin, request) = server("HTTP/1.1 200 OK\r\nContent-Length: 20\r\n\r\n", false);
    let source = format!(
        "export function run(ctx) {{ try {{ ctx.network.request({{url:'{origin}/import',method:'POST'}}); }} catch {{}} ctx.state.save('caught'); }}"
    );
    let fixture = Fixture::new(&source, &origin, true);
    fixture.configure();
    fixture.start();
    request.recv_timeout(Duration::from_secs(4)).unwrap();
    let snapshot = fixture.wait(JobPhase::Interrupted);
    assert!(!snapshot.jobs[0].configuration.as_ref().unwrap().enabled);
    assert_eq!(
        serde_json::from_str::<Value>(&fixture.journal()).unwrap()["jobs"][0]["state"],
        "caught"
    );
    assert!(
        fixture
            .client
            .plugin_jobs(JobCommand::RunNow {
                plugin_id: "example".into(),
                job_id: "sync".into()
            })
            .is_err()
    );
}

#[test]
fn successful_external_mutation_followed_by_handler_failure_requires_inspection() {
    let (origin, request) = server("HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}", false);
    let source = format!(
        "export function run(ctx) {{ ctx.network.request({{url:'{origin}/import',method:'POST'}}); throw Error('after external effect'); }}"
    );
    let fixture = Fixture::new(&source, &origin, true);
    fixture.configure();
    fixture.start();
    request.recv_timeout(Duration::from_secs(4)).unwrap();
    fixture.wait(JobPhase::Interrupted);
}

#[test]
fn redirects_are_returned_as_failure_without_dispatching_the_target() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let reply = Box::leak(
        format!(
            "HTTP/1.1 302 Found\r\nLocation: http://{}/secret\r\nContent-Length: 0\r\n\r\n",
            listener.local_addr().unwrap()
        )
        .into_boxed_str(),
    );
    let (origin, request) = server(reply, false);
    let source = format!(
        "export function run(ctx) {{ ctx.network.request({{url:'{origin}/transactions',credential:'bank'}}); }}"
    );
    let fixture = Fixture::new(&source, &origin, true);
    fixture.secret(Some("synthetic-key"));
    fixture.configure();
    fixture.start();
    request.recv_timeout(Duration::from_secs(4)).unwrap();
    fixture.wait(JobPhase::Failed);
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[test]
fn job_custody_names_cannot_alias_another_vault_plugin_or_job() {
    let id = Uuid::new_v4();
    let namespace = lapis_app_core::services::job_credential_namespace;
    let own = namespace(id, "example", "sync");
    assert!(
        lapis_app_core::credentials::PluginGrants::parse(own.as_str()).is_err(),
        "Background custody cannot be a legal foreground plugin identity"
    );
    for other in [
        namespace(Uuid::new_v4(), "example", "sync"),
        namespace(id, "other", "sync"),
        namespace(id, "example", "other"),
    ] {
        assert!(own != other);
    }
}

#[test]
fn cold_journal_grants_cannot_exceed_the_verified_declaration() {
    let target = TcpListener::bind("127.0.0.1:0").unwrap();
    target.set_nonblocking(true).unwrap();
    let origin = format!("http://{}", target.local_addr().unwrap());
    let source = format!(
        "export function run(ctx) {{ ctx.network.request({{url:'{origin}/transactions'}}); }}"
    );
    let mut fixture = Fixture::new(&source, "http://127.0.0.1:1", true);
    fixture.configure();
    fixture.runtime.close_vault(fixture.id).unwrap();
    let directory = fixture
        .profile
        .path()
        .join("jobs")
        .join(fixture.id.to_string());
    let path = fs::read_dir(directory)
        .unwrap()
        .map(Result::unwrap)
        .find(|entry| entry.path().extension().is_some_and(|e| e == "json"))
        .unwrap()
        .path();
    let mut catalog: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    catalog["jobs"][0]["configuration"]["permissions"]["network"][0]["origin"] = json!(origin);
    fs::write(path, serde_json::to_vec(&catalog).unwrap()).unwrap();
    fixture.client = fixture
        .runtime
        .admit(fixture.id, fixture.vault.path())
        .unwrap()
        .attach()
        .unwrap();
    assert!(
        fixture
            .client
            .plugin_jobs(JobCommand::RunNow {
                plugin_id: "example".into(),
                job_id: "sync".into()
            })
            .is_err()
    );
    assert_eq!(
        target.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[test]
fn oversized_responses_and_literal_credential_reflection_fail_without_checkpoint_leaks() {
    for reply in [
        "HTTP/1.1 200 OK\r\nContent-Length: 131073\r\n\r\n",
        "HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\nsynthetic-key",
    ] {
        let (origin, request) = server(reply, false);
        let source = format!(
            "export function run(ctx) {{ const r=ctx.network.request({{url:'{origin}/transactions',credential:'bank'}}); ctx.state.save(r.body); }}"
        );
        let fixture = Fixture::new(&source, &origin, true);
        fixture.secret(Some("synthetic-key"));
        fixture.configure();
        fixture.start();
        request.recv_timeout(Duration::from_secs(4)).unwrap();
        fixture.wait(JobPhase::Failed);
        assert!(!fixture.journal().contains("synthetic-key"));
        assert!(
            serde_json::from_str::<Value>(&fixture.journal()).unwrap()["jobs"][0]["state"]
                .is_null()
        );
    }
}

#[test]
fn missing_credentials_fail_before_dispatch_and_private_approval_is_required() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let source = format!(
        "export function run(ctx) {{ ctx.network.request({{url:'{origin}/transactions',credential:'bank'}}); }}"
    );
    let fixture = Fixture::new(&source, &origin, true);
    fixture.configure();
    fixture.start();
    fixture.wait(JobPhase::Failed);
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    let origin = format!("https://{}", listener.local_addr().unwrap());
    let source = format!(
        "export function run(ctx) {{ ctx.network.request({{url:'{origin}/transactions'}}); }}"
    );
    let fixture = Fixture::new(&source, &origin, false);
    fixture.configure();
    fixture.start();
    fixture.wait(JobPhase::Failed);
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}
