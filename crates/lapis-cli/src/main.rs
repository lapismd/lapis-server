mod args;
mod client;
mod credential;
mod desktop;
mod errors;
mod installation;
mod mcp;
mod operations;
mod output;
mod validation;

use clap::Parser;
use lapis_app_protocol::content::*;
use std::process::ExitCode;
use uuid::Uuid;

fn main() -> ExitCode {
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        use std::io::Write;
        let _ = writeln!(std::io::stderr(), "Cannot start CLI runtime");
        return ExitCode::from(7);
    };
    let result = runtime.block_on(run());
    // MCP drains its async services before returning. Tokio's stdin reader uses
    // blocking IO that cannot be cancelled when a client closes only stdout;
    // do not let that reader prevent this standalone process from exiting.
    runtime.shutdown_background();
    result
}

async fn run() -> ExitCode {
    let mut args = args::Cli::parse();
    if let args::Command::Cli { command } = &args.command {
        return installation::run(command, args.json);
    }
    if let Err(error) = desktop::resolve(&mut args) {
        let code = errors::exit_code(error.code);
        let reply = ContentReply {
            api_version: CONTENT_API_MAJOR,
            request_id: Uuid::new_v4(),
            outcome: ContentOutcome::Error { error },
        };
        // MCP stdout is exclusively reserved for protocol frames.
        if matches!(args.command, args::Command::Mcp { .. }) {
            use std::io::Write;
            let _ = writeln!(
                std::io::stderr(),
                "Desktop access unavailable; enable access and create a credential in Desktop settings"
            );
        } else {
            let _ = output::write(&reply, args.json);
        }
        return ExitCode::from(code);
    }
    if matches!(
        args.command,
        args::Command::Operation(_) | args::Command::Help { .. }
    ) {
        return operations::run(&args).await;
    }
    let Some(command) = args.command.content() else {
        return match mcp::run(&args).await {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                use std::io::Write;
                let _ = writeln!(std::io::stderr(), "{}", error.message);
                ExitCode::from(errors::exit_code(error.code))
            }
        };
    };
    let request = ContentRequest {
        api_version: CONTENT_API_MAJOR,
        request_id: Uuid::new_v4(),
        command,
    };
    let reply = match client::execute(&args, &request).await {
        Ok(reply) => reply,
        Err(error) => ContentReply {
            api_version: CONTENT_API_MAJOR,
            request_id: request.request_id,
            outcome: ContentOutcome::Error { error },
        },
    };
    let code = match &reply.outcome {
        ContentOutcome::Ok { .. } => 0,
        ContentOutcome::Error { error } => errors::exit_code(error.code),
    };
    match output::write(&reply, args.json) {
        Ok(()) => ExitCode::from(code),
        Err(error) if error.kind() == std::io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(_) => {
            use std::io::Write;
            let _ = writeln!(std::io::stderr(), "Cannot write CLI output");
            ExitCode::from(7)
        }
    }
}
