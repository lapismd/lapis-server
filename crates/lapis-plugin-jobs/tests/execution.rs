use lapis_plugin_jobs::{ExecutionLimits, JobHost, execute};
use serde_json::{Value, json};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

#[derive(Default)]
struct Host {
    calls: Mutex<Vec<String>>,
    cancelled: AtomicBool,
}
impl JobHost for Host {
    fn invoke(&self, action: &str, input: Value) -> Result<Value, String> {
        self.calls.lock().unwrap().push(action.into());
        match action {
            "state.save" => Ok(input),
            "state.load" => Ok(json!({"cursor": 17})),
            "cancelled" => Ok(Value::Bool(self.cancelled())),
            _ => Err("Permission denied".into()),
        }
    }
    fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

fn run(source: &str, host: Arc<Host>) -> Result<(), String> {
    execute(
        source,
        &json!({"runId": "run-1"}),
        host,
        ExecutionLimits::default(),
    )
}

#[test]
fn runs_installed_esm_with_only_explicit_context() {
    let host = Arc::new(Host::default());
    run("export function run(ctx) { if(ctx.runId !== 'run-1') throw Error(); ctx.state.save(ctx.state.load()); }", host.clone()).unwrap();
    assert_eq!(*host.calls.lock().unwrap(), ["state.load", "state.save"]);
}

#[test]
fn awaits_async_handlers_and_rejects_failed_promises() {
    let host = Arc::new(Host::default());
    run(
        "export async function run(ctx) { await Promise.resolve(); ctx.state.save(1); }",
        host.clone(),
    )
    .unwrap();
    assert_eq!(*host.calls.lock().unwrap(), ["state.save"]);
    assert!(
        run(
            "export async function run() { throw Error('failed'); }",
            host
        )
        .is_err()
    );
}

#[test]
fn exposes_no_filesystem_network_process_or_renderer_authority() {
    run("export function run() { for(const key of ['process','require','fetch','window','document','__jobInvoke','__jobContext']) { if(typeof globalThis[key] !== 'undefined') throw Error(key); } }", Arc::new(Host::default())).unwrap();
    assert!(
        run(
            "import fs from 'node:fs'; export function run() { fs.readFileSync('/etc/passwd'); }",
            Arc::new(Host::default())
        )
        .is_err()
    );
}

#[test]
fn checks_host_permission_for_each_call() {
    let host = Arc::new(Host::default());
    assert!(
        run(
            "export function run(ctx) { ctx.notes.frontmatter('private.md'); }",
            host.clone()
        )
        .is_err()
    );
    assert_eq!(*host.calls.lock().unwrap(), ["notes.frontmatter"]);
}

#[test]
fn bounds_calls_and_serialized_arguments() {
    let host = Arc::new(Host::default());
    let limits = ExecutionLimits {
        max_calls: 2,
        ..Default::default()
    };
    assert!(
        execute(
            "export function run(ctx) { for(let i=0;i<3;i++)ctx.state.load(); }",
            &json!({}),
            host.clone(),
            limits
        )
        .is_err()
    );
    assert_eq!(host.calls.lock().unwrap().len(), 2);
    let host = Arc::new(Host::default());
    assert!(
        run(
            "export function run(ctx) { ctx.state.save('a'.repeat(1048577)); }",
            host.clone()
        )
        .is_err()
    );
    assert!(host.calls.lock().unwrap().is_empty());
}

#[test]
fn interrupts_cpu_bound_handlers_and_module_initialization() {
    for source in [
        "export function run() { while(true) {} }",
        "while(true) {} export function run() {}",
    ] {
        let start = Instant::now();
        let limits = ExecutionLimits {
            duration: Duration::from_millis(25),
            ..Default::default()
        };
        assert!(execute(source, &json!({}), Arc::new(Host::default()), limits).is_err());
        assert!(start.elapsed() < Duration::from_secs(3));
    }
}

#[test]
fn cancels_a_running_handler_without_waiting_for_a_host_call() {
    let host = Arc::new(Host::default());
    let worker = host.clone();
    let thread =
        std::thread::spawn(move || run("export function run() { while(true) {} }", worker));
    std::thread::sleep(Duration::from_millis(20));
    host.cancelled.store(true, Ordering::Release);
    assert!(thread.join().unwrap().is_err());
}

#[test]
fn bounds_memory_and_rejects_missing_handlers() {
    assert!(run("export function run() { const values=[]; while(true) values.push(new Array(10000).fill('value')); }", Arc::new(Host::default())).is_err());
    assert!(run("export const other = 1;", Arc::new(Host::default())).is_err());
    assert!(run("export function run() {}", Arc::new(Host::default())).is_ok());
}

struct ReturningHost {
    invoked: AtomicBool,
    cancelled: AtomicBool,
    cancel_on_return: bool,
}
impl JobHost for ReturningHost {
    fn invoke(&self, _action: &str, _input: Value) -> Result<Value, String> {
        self.invoked.store(true, Ordering::Release);
        if self.cancel_on_return {
            self.cancelled.store(true, Ordering::Release);
        } else {
            std::thread::sleep(Duration::from_millis(200));
        }
        Ok(Value::Null)
    }
    fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

#[test]
fn refuses_success_after_native_callback_exceeds_deadline() {
    callback_cannot_report_success(false);
}

#[test]
fn refuses_success_after_native_callback_requests_cancellation() {
    callback_cannot_report_success(true);
}

fn callback_cannot_report_success(cancel_on_return: bool) {
    let host = Arc::new(ReturningHost {
        invoked: AtomicBool::new(false),
        cancelled: AtomicBool::new(false),
        cancel_on_return,
    });
    let result = execute(
        "export function run(ctx) { try { ctx.state.load(); } catch {} }",
        &json!({}),
        host.clone(),
        ExecutionLimits {
            duration: Duration::from_millis(if cancel_on_return { 1000 } else { 100 }),
            ..Default::default()
        },
    );
    assert!(host.invoked.load(Ordering::Acquire));
    assert!(
        result.is_err(),
        "A terminal native callback reported success"
    );
}
