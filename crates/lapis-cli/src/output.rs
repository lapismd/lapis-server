use lapis_app_protocol::content::*;
use std::io::{self, Write};

pub(crate) fn write(reply: &ContentReply, json: bool) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    if json {
        serde_json::to_writer(&mut stdout, reply).map_err(|error| {
            io::Error::new(error.io_error_kind().unwrap_or(io::ErrorKind::Other), error)
        })?;
        stdout.write_all(b"\n")?;
    } else if let ContentOutcome::Ok { value } = &reply.outcome {
        match value {
            ContentValue::Note { text, .. } => stdout.write_all(text.as_bytes())?,
            ContentValue::Vaults { items, next_cursor } => {
                for item in items {
                    writeln!(stdout, "{}\t{}", item.vault_id, item.name)?;
                }
                continuation(&mut stderr, next_cursor).map_err(io::Error::other)?;
            }
            ContentValue::Notes { items, next_cursor } => {
                for item in items {
                    writeln!(stdout, "{}\t{}", item.path, item.size_bytes)?;
                }
                continuation(&mut stderr, next_cursor).map_err(io::Error::other)?;
            }
            ContentValue::Capabilities { capabilities } => {
                for op in &capabilities.operations {
                    let id = serde_json::to_value(op.operation).map_err(io::Error::other)?;
                    writeln!(stdout, "{}", id.as_str().unwrap_or("unknown"))?;
                }
            }
            _ => return Err(io::Error::other("Unexpected content result")),
        }
    }
    if let ContentOutcome::Error { error } = &reply.outcome {
        writeln!(stderr, "{:?}: {}", error.code, error.message).map_err(io::Error::other)?;
    }
    stdout.flush()
}

fn continuation(stderr: &mut impl Write, cursor: &Option<String>) -> io::Result<()> {
    if let Some(cursor) = cursor {
        writeln!(stderr, "Next page: --cursor {cursor}")?;
    }
    Ok(())
}
