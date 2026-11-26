use crate::JobHost;
use rquickjs::{Context, Function, Module, Object, Runtime, Value, function::Func};
use serde_json::Value as Json;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

#[derive(Clone, Copy)]
pub struct ExecutionLimits {
    pub memory_bytes: usize,
    pub duration: Duration,
    pub max_calls: usize,
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        Self {
            memory_bytes: 16 * 1024 * 1024,
            duration: Duration::from_secs(30),
            max_calls: 1024,
        }
    }
}

/// A fresh engine for each run. Only the broker crosses into the native host.
pub fn execute(
    source: &str,
    context: &Json,
    host: Arc<dyn JobHost>,
    limits: ExecutionLimits,
) -> Result<(), String> {
    if source.len() > 1024 * 1024
        || limits.memory_bytes < 1024 * 1024
        || limits.memory_bytes > 64 * 1024 * 1024
        || limits.duration > Duration::from_secs(60)
        || limits.duration.is_zero()
        || !(1..=4096).contains(&limits.max_calls)
    {
        return Err("Plugin job exceeds execution bounds".into());
    }
    let deadline = Instant::now()
        .checked_add(limits.duration)
        .ok_or("Invalid job deadline")?;
    let runtime = Runtime::new().map_err(engine_error)?;
    runtime.set_memory_limit(limits.memory_bytes);
    runtime.set_max_stack_size(256 * 1024);
    let interrupt_host = host.clone();
    runtime.set_interrupt_handler(Some(Box::new(move || {
        Instant::now() >= deadline || interrupt_host.cancelled()
    })));
    let engine = Context::full(&runtime).map_err(engine_error)?;
    let completion_host = host.clone();
    engine
        .with(|ctx| {
            let calls = std::sync::atomic::AtomicUsize::new(0);
            let invoke = Func::from(move |action: String, input: String| -> String {
                let result: Result<Json, String> = (|| {
                    if Instant::now() >= deadline || host.cancelled() {
                        return Err("Plugin job was cancelled".into());
                    }
                    if calls.fetch_add(1, std::sync::atomic::Ordering::Relaxed) >= limits.max_calls
                        || action.len() > 64
                        || input.len() > 1024 * 1024
                    {
                        return Err("Plugin job call exceeds bounds".into());
                    }
                    let input =
                        serde_json::from_str(&input).map_err(|_| "Invalid plugin job input")?;
                    let output = host.invoke_until(&action, input, deadline)?;
                    if serde_json::to_vec(&output)
                        .map_err(|_| "Invalid job result")?
                        .len()
                        > 1024 * 1024
                    {
                        return Err("Plugin job result exceeds bounds".into());
                    }
                    Ok(output)
                })();
                match result {
                    Ok(value) => serde_json::json!({"value": value}).to_string(),
                    Err(error) => serde_json::json!({"error": error}).to_string(),
                }
            });
            ctx.globals()
                .set("__jobInvoke", invoke)
                .map_err(engine_error)?;
            ctx.globals()
                .set("__jobContext", context.to_string())
                .map_err(engine_error)?;
            let argument: Object = ctx.eval(include_str!("context.js")).map_err(engine_error)?;
            let (module, initialized) = Module::declare(ctx.clone(), "plugin-job", source)
                .and_then(Module::eval)
                .map_err(engine_error)?;
            initialized.finish::<()>().map_err(engine_error)?;
            let run: Function = module.get("run").map_err(engine_error)?;
            let result: Value = run.call((argument,)).map_err(engine_error)?;
            if let Some(promise) = result.as_promise() {
                promise.finish::<()>().map_err(engine_error)?;
            }
            Ok(())
        })
        .and_then(|()| {
            // An admitted native operation settles before its execution lease ends.
            // Neither its final callback nor a handler catch can hide a terminal budget.
            if Instant::now() >= deadline {
                Err("Plugin job exceeded its execution deadline".into())
            } else if completion_host.cancelled() {
                Err("Plugin job was cancelled".into())
            } else {
                Ok(())
            }
        })
}

fn engine_error(error: rquickjs::Error) -> String {
    // Plugin exceptions can contain authored data. Keep public diagnostics bounded.
    format!("Plugin job execution failed ({error})")
}
