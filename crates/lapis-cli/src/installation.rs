use crate::args::Installation;
use lapis_cli::install::{self, Shell, Status};
use std::{io, path::PathBuf, process::ExitCode};

pub(crate) fn run(command: &Installation, json: bool) -> ExitCode {
    match execute(command) {
        Ok((state, destination)) => {
            let (label, state) = match state {
                Status::Installed => ("Installed", "installed"),
                Status::NotInstalled => ("Not installed", "not-installed"),
                Status::Broken => ("Needs repair", "broken"),
                Status::Conflict => ("Conflicting command", "conflict"),
            };
            if json {
                println!(
                    "{}",
                    serde_json::json!({ "state": state, "destination": destination, "version": env!("CARGO_PKG_VERSION") })
                );
            } else {
                println!("{label}: {}", destination.display());
                if state == "installed" {
                    println!("Open a new terminal and run: command -v lapis");
                }
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("CLI setup failed: {error}");
            ExitCode::from(6)
        }
    }
}
fn execute(command: &Installation) -> io::Result<(Status, PathBuf)> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or_else(|| io::Error::other("An absolute HOME is required"))?;
    let state = match command {
        Installation::Status => install::status(&home)?,
        Installation::Install {
            setup_path,
            replace,
        } => {
            let shell = setup_path.as_deref().map(|value| {
                if value == "zsh" {
                    Shell::Zsh
                } else {
                    Shell::Bash
                }
            });
            install::install(&home, &std::env::current_exe()?, shell, *replace)?
        }
        Installation::Uninstall { remove_path } => {
            install::uninstall(&home, *remove_path)?;
            Status::NotInstalled
        }
    };
    Ok((state, install::destination(&home)))
}
