use super::unavailable;
use crate::services::Result;
use ort::environment::Environment;
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

// ONNX permits only one environment per process, even after sessions close.
// This is the installed inference library, not a vault/service writer owner.
static ENVIRONMENT: Mutex<Option<(PathBuf, Environment)>> = Mutex::new(None);

pub(super) fn load(path: &Path) -> Result<Environment> {
    let path = path
        .canonicalize()
        .map_err(|_| unavailable("Native ONNX runtime is unavailable"))?;
    let mut environment = ENVIRONMENT
        .lock()
        .map_err(|_| unavailable("Native ONNX environment is unavailable"))?;
    if let Some((installed, environment)) = environment.as_ref() {
        if installed != &path {
            return Err(unavailable(
                "Native ONNX runtime differs from the initialized installation",
            ));
        }
        return Ok(environment.clone());
    }
    let initialized = ort::init_from(&path)
        .map_err(|_| unavailable("Native ONNX runtime could not be loaded"))?
        .with_name("lapis-search")
        .with_telemetry(false)
        .build()
        .map_err(|_| unavailable("Native ONNX environment could not start"))?;
    *environment = Some((path, initialized.clone()));
    Ok(initialized)
}
