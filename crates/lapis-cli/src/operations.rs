//! A data-driven command tree; no domain-specific dispatch lives here.
use crate::{
    args::{Cli, Command},
    client, credential, errors,
};
use clap::{Arg, Command as ClapCommand, CommandFactory};
use lapis_app_protocol::{content::*, operations::*};
use serde_json::Value;
use std::{io::Write, process::ExitCode, time::Duration};
use uuid::Uuid;
mod input;

fn invalid() -> ContentError {
    errors::failure(ContentErrorCode::InvalidRequest)
}

pub(crate) fn overview() -> String {
    let catalog: Vec<OperationDescriptor> =
        serde_json::from_str(include_str!("../help/tasks-operations.json"))
            .expect("Bundled operation catalog is valid");
    let roots = catalog
        .into_iter()
        .map(|d| d.plugin_id)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "Plugin commands: {roots}\nUse 'lapis help <plugin>' or 'lapis help -k <keyword>' for authored guides.\nHost and credential flags precede a plugin command."
    )
}

async fn fetch(
    args: &Cli,
    path: &str,
    body: Option<&OperationRequest>,
) -> Result<Value, ContentError> {
    let mut url = client::target(args.target.as_deref())?;
    url.set_path(path);
    let token = credential::read(args)?;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .timeout(Duration::from_millis(args.timeout_ms))
        .build()
        .map_err(|_| errors::failure(ContentErrorCode::Unavailable))?;
    let request_id = body.map(|b| b.request_id).unwrap_or_else(Uuid::new_v4);
    let mut request = if body.is_some() {
        client.post(url)
    } else {
        client.get(url)
    };
    if let Some(body) = body {
        request = request
            .header("content-type", "application/json")
            .body(serde_json::to_vec(body).map_err(|_| invalid())?);
    }
    let mut response = request
        .bearer_auth(token)
        .header("x-lapis-content-api", "1")
        .header("x-lapis-request-id", request_id.to_string())
        .send()
        .await
        .map_err(|_| ContentError {
            code: if body.is_some() {
                ContentErrorCode::OutcomeUnknown
            } else {
                ContentErrorCode::Unavailable
            },
            message: format!("Transport interrupted; inspect request {request_id} before retrying"),
            effect: if body.is_some() {
                ContentEffectStatus::Unknown
            } else {
                ContentEffectStatus::NotStarted
            },
        })?;
    if response.status().is_redirection()
        || response
            .headers()
            .get("x-lapis-content-api")
            .and_then(|v| v.to_str().ok())
            != Some("1")
        || response
            .headers()
            .get("x-lapis-request-id")
            .and_then(|v| v.to_str().ok())
            != Some(request_id.to_string().as_str())
    {
        return Err(errors::failure(ContentErrorCode::ProtocolMismatch));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| errors::failure(ContentErrorCode::OutcomeUnknown))?
    {
        if bytes.len() + chunk.len() > 2 * 1024 * 1024 {
            return Err(errors::failure(ContentErrorCode::TooLarge));
        }
        bytes.extend_from_slice(&chunk);
    }
    let value: Value = serde_json::from_slice(&bytes)
        .map_err(|_| errors::failure(ContentErrorCode::ProtocolMismatch))?;
    if value.get("api_version").is_some() && value["outcome"]["status"] == "error" {
        let mut error: ContentError = serde_json::from_value(value["outcome"]["error"].clone())
            .map_err(|_| errors::failure(ContentErrorCode::ProtocolMismatch))?;
        error.message = errors::message(error.code).into();
        return Err(error);
    }
    Ok(value)
}

fn command(plugin: &str, descriptors: &[OperationDescriptor]) -> ClapCommand {
    let mut root = ClapCommand::new(plugin.to_owned())
        .bin_name(format!("lapis {plugin}"))
        .disable_help_subcommand(true)
        .arg(
            Arg::new("vault")
                .long("vault")
                .help("Explicit admitted vault UUID")
                .global(true),
        )
        .arg(
            Arg::new("invocation-id")
                .long("invocation-id")
                .help("Stable request UUID for explicit lifecycle inspection")
                .global(true),
        )
        .arg(
            Arg::new("input-file")
                .long("input-file")
                .help("JSON input object file, at most 64 KiB; explicit flags override its fields")
                .global(true),
        );
    for descriptor in descriptors.iter().filter(|d| d.plugin_id == plugin) {
        let mut child = ClapCommand::new(descriptor.id.clone())
            .about(descriptor.help.summary.clone())
            .long_about(format!(
                "{}\n\n{}",
                descriptor.help.markdown,
                descriptor.help.examples.join("\n")
            ));
        for argument in &descriptor.help.arguments {
            let mut arg = Arg::new(argument.field.clone())
                .long(argument.flag.clone())
                .help(argument.description.clone());
            if argument.required {
                arg = arg.required_unless_present("input-file");
            }
            child = child.arg(arg);
        }
        root = root.subcommand(child);
    }
    root
}

async fn execute(args: &Cli) -> Result<u8, ContentError> {
    let bundled_help = match &args.command {
        Command::Help { words, .. } => {
            words.is_empty() || words.first().is_some_and(|word| word == "tasks")
        }
        Command::Operation(tokens) => {
            tokens.first().is_some_and(|word| word == "tasks")
                && tokens.iter().any(|word| word == "--help" || word == "-h")
        }
        _ => false,
    };
    let catalog: OperationCatalog = if args.target.is_some() && !bundled_help {
        serde_json::from_value(fetch(args, "/api/content/v1/operations", None).await?)
            .map_err(|_| errors::failure(ContentErrorCode::ProtocolMismatch))?
    } else {
        OperationCatalog {
            protocol_version: 1,
            operations: serde_json::from_str(include_str!("../help/tasks-operations.json"))
                .map_err(|_| invalid())?,
        }
    };
    if catalog.protocol_version != 1 {
        return Err(errors::failure(ContentErrorCode::ProtocolMismatch));
    }
    let (tokens, help, keyword) = match &args.command {
        Command::Operation(tokens) => (tokens.clone(), false, None),
        Command::Help { words, keyword } => (words.clone(), true, keyword.as_deref()),
        _ => unreachable!(),
    };
    if let Some(keyword) = keyword {
        for descriptor in &catalog.operations {
            if descriptor.help.keywords.iter().any(|k| k.contains(keyword)) {
                println!(
                    "{}\n{}",
                    descriptor.help.command.join(" "),
                    descriptor.help.markdown
                );
            }
        }
        return Ok(0);
    }
    let Some(plugin) = tokens.first() else {
        Cli::command().print_long_help().map_err(|_| invalid())?;
        return Ok(0);
    };
    let mut tree = command(plugin, &catalog.operations);
    if !catalog.operations.iter().any(|d| &d.plugin_id == plugin) {
        return Err(errors::failure(ContentErrorCode::Unsupported));
    }
    let mut tokens = tokens.clone();
    if help {
        tokens.push("--help".into());
    }
    let matches = match tree.try_get_matches_from_mut(tokens) {
        Ok(matches) => matches,
        Err(error) => {
            let code = error.exit_code() as u8;
            error.print().map_err(|_| invalid())?;
            return Ok(code);
        }
    };
    let (id, matches) = matches.subcommand().ok_or_else(invalid)?;
    let descriptor = catalog
        .operations
        .iter()
        .find(|d| &d.plugin_id == plugin && d.id == id)
        .ok_or_else(invalid)?;
    let vault = Uuid::parse_str(matches.get_one::<String>("vault").ok_or_else(invalid)?)
        .map_err(|_| invalid())?;
    let request_id = matches
        .get_one::<String>("invocation-id")
        .map(|v| Uuid::parse_str(v))
        .transpose()
        .map_err(|_| invalid())?
        .unwrap_or_else(Uuid::new_v4);
    let request = OperationRequest {
        protocol_version: 1,
        request_id,
        version: descriptor.version,
        input: input::read(descriptor, matches)?,
    };
    let path = format!("/api/content/v1/vaults/{vault}/plugins/{plugin}/operations/{id}");
    let reply: OperationReply = serde_json::from_value(fetch(args, &path, Some(&request)).await?)
        .map_err(|_| errors::failure(ContentErrorCode::ProtocolMismatch))?;
    if reply.request_id != request_id || reply.protocol_version != 1 {
        return Err(errors::failure(ContentErrorCode::ProtocolMismatch));
    }
    let code = match &reply.outcome {
        OperationOutcome::Ok { .. } => 0,
        OperationOutcome::Error { error } => errors::exit_code(error.code),
    };
    if args.json {
        serde_json::to_writer(std::io::stdout(), &reply).map_err(|_| invalid())?;
        println!();
    } else {
        match reply.outcome {
            OperationOutcome::Ok { value } => println!(
                "{}",
                serde_json::to_string_pretty(&value).map_err(|_| invalid())?
            ),
            OperationOutcome::Error { error } => {
                eprintln!("{} (request {request_id})", error.message)
            }
        }
    }
    Ok(code)
}

pub async fn run(args: &Cli) -> ExitCode {
    match execute(args).await {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            let _ = writeln!(std::io::stderr(), "{}", error.message);
            ExitCode::from(errors::exit_code(error.code))
        }
    }
}
