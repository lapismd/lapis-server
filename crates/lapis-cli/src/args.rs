use clap::{Args, Parser, Subcommand};
use lapis_app_protocol::content::*;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Parser)]
#[command(
    name = "lapis",
    version,
    about = "Read notes through an explicitly selected Lapis content host"
)]
pub(crate) struct Cli {
    /// Exact HTTPS origin, or HTTP loopback for a local host.
    #[arg(long, global = true)]
    pub target: Option<String>,
    /// Read the scoped bearer credential from a protected file (Unix).
    #[arg(long, global = true, conflicts_with = "token_stdin")]
    pub token_file: Option<PathBuf>,
    /// Read the scoped bearer credential from stdin until EOF.
    #[arg(long, global = true)]
    pub token_stdin: bool,
    /// Emit one stable ContentReply JSON value to stdout.
    #[arg(long, global = true)]
    pub json: bool,
    /// Maximum time for the complete HTTP request and response.
    #[arg(long, global = true, default_value_t = 15000, value_parser = clap::value_parser!(u64).range(1..=120000))]
    pub timeout_ms: u64,
    #[command(subcommand)]
    pub command: Command,
}
#[derive(Subcommand)]
pub(crate) enum Command {
    /// Show operations advertised by the selected host and credential.
    Capabilities,
    /// Discover admitted vault identities.
    Vault {
        #[command(subcommand)]
        command: Vault,
    },
    /// List or read saved notes in an admitted vault.
    Note {
        #[command(subcommand)]
        command: Note,
    },
    /// Bridge an MCP subprocess client to the selected content host.
    Mcp {
        #[command(subcommand)]
        command: Mcp,
    },
}
#[derive(Subcommand)]
pub(crate) enum Mcp {
    Serve {
        /// Reserve stdin/stdout for MCP; requires --token-file.
        #[arg(long, required = true)]
        stdio: bool,
    },
}
#[derive(Subcommand)]
pub(crate) enum Vault {
    List(Page),
}
#[derive(Subcommand)]
pub(crate) enum Note {
    List {
        #[arg(long)]
        vault: Uuid,
        #[arg(long)]
        directory: Option<String>,
        #[command(flatten)]
        page: Page,
    },
    Read {
        #[arg(long)]
        vault: Uuid,
        #[arg(long)]
        path: String,
    },
}
#[derive(Args)]
pub(crate) struct Page {
    #[arg(long, default_value_t = 50, value_parser = clap::value_parser!(u16).range(1..=100))]
    limit: u16,
    #[arg(long)]
    cursor: Option<String>,
}
impl Page {
    fn query(&self) -> ContentPageQuery {
        ContentPageQuery {
            limit: self.limit,
            cursor: self.cursor.clone(),
        }
    }
}
impl Command {
    pub(crate) fn content(&self) -> Option<ContentCommand> {
        Some(match self {
            Self::Mcp { .. } => return None,
            Self::Capabilities => ContentCommand::Capabilities {},
            Self::Vault {
                command: Vault::List(page),
            } => ContentCommand::ListVaults { page: page.query() },
            Self::Note {
                command:
                    Note::List {
                        vault,
                        directory,
                        page,
                    },
            } => ContentCommand::ListNotes {
                vault_id: *vault,
                directory: directory.clone(),
                page: page.query(),
            },
            Self::Note {
                command: Note::Read { vault, path },
            } => ContentCommand::ReadNote {
                vault_id: *vault,
                path: path.clone(),
            },
        })
    }
}
