use anyhow::{Context, Result, ensure};
use lapis_container_mods::{config::Config, registry::Registry, runtime};
use std::{ffi::CString, os::unix::process::CommandExt, process::Command};

fn main() {
    if let Err(error) = run() {
        lapis_container_mods::log("fatal", &error.to_string());
        std::process::exit(70);
    }
}
fn run() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mut uid = None;
    let mut gid = None;
    let command = loop {
        match args.next().as_deref() {
            Some("--uid") => uid = Some(args.next().context("missing uid")?.parse::<u32>()?),
            Some("--gid") => gid = Some(args.next().context("missing gid")?.parse::<u32>()?),
            Some("--") => break args.collect::<Vec<_>>(),
            _ => anyhow::bail!(
                "usage: mod-loader [--uid UID --gid GID] -- ORIGINAL_ENTRYPOINT [ARGS]"
            ),
        }
    };
    ensure!(
        !command.is_empty() && uid.is_some() == gid.is_some(),
        "entrypoint and paired UID/GID required"
    );
    let config = Config::from_env()?;
    if !config.references.is_empty() {
        // PID 1 ignores default termination while initialization is in progress.
        // Exit promptly even if a shared-cache lock or synchronous hook is waiting.
        unsafe {
            ensure!(
                libc::signal(
                    libc::SIGTERM,
                    stop_initialization as *const () as libc::sighandler_t
                ) != libc::SIG_ERR,
                "cannot handle initialization termination"
            );
            ensure!(
                libc::signal(
                    libc::SIGINT,
                    stop_initialization as *const () as libc::sighandler_t
                ) != libc::SIG_ERR,
                "cannot handle initialization interrupt"
            );
        }
        let registry = Registry::production()?;
        tokio::runtime::Runtime::new()?.block_on(runtime::load(&config, &registry))?;
        // Leave inherited signal dispositions untouched on the no-Mod fast path.
        unsafe {
            libc::signal(libc::SIGTERM, libc::SIG_DFL);
            libc::signal(libc::SIGINT, libc::SIG_DFL);
        }
    }
    if let (Some(uid), Some(gid)) = (uid, gid) {
        // SAFETY: setgroups/setgid/setuid are called once, before application execution.
        if unsafe { libc::geteuid() } == 0 {
            ensure!(
                unsafe { libc::setgroups(0, std::ptr::null()) } == 0,
                "cannot drop supplemental groups"
            );
            ensure!(
                unsafe { libc::setgid(gid) } == 0 && unsafe { libc::setuid(uid) } == 0,
                "cannot drop application privileges"
            );
        } else {
            ensure!(
                unsafe { libc::geteuid() } == uid && unsafe { libc::getegid() } == gid,
                "cannot switch unprivileged identity"
            );
        }
    }
    // exec preserves PID 1, argv boundaries and signal handling; no shell evaluation.
    let _ = CString::new(command[0].as_bytes())?;
    Err(Command::new(&command[0]).args(&command[1..]).exec().into())
}

extern "C" fn stop_initialization(signal: libc::c_int) {
    // _exit is async-signal-safe. Verified cache/file publication uses atomic rename;
    // stopping initialization never starts the application or leaves a daemon.
    unsafe { libc::_exit(128 + signal) }
}
