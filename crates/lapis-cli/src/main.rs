mod args;
mod client;
mod credential;
mod errors;
mod output;
mod validation;

use clap::Parser;
use lapis_app_protocol::content::*;
use std::process::ExitCode;
use uuid::Uuid;

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let args = args::Cli::parse();
    let request = ContentRequest {
        api_version: CONTENT_API_MAJOR,
        request_id: Uuid::new_v4(),
        command: args.command.content(),
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
