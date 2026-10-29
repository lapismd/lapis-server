use crate::services::{Error, Result};
use lapis_app_protocol::{ErrorCode, history_index::*};

fn invalid() -> Error {
    Error::new(ErrorCode::InvalidRequest, "Invalid bounded History request")
}

pub(super) fn validate(command: &HistoryIndexCommand) -> Result<()> {
    match command {
        HistoryIndexCommand::Query { search } => {
            if search.limit == 0
                || search.limit > 100
                || search
                    .cursor
                    .as_ref()
                    .is_some_and(|cursor| cursor.len() > 512)
            {
                return Err(invalid());
            }
            expression(&search.expression, 0, &mut 0)?;
        }
        HistoryIndexCommand::Context {
            commit,
            path,
            start,
            end,
        } => {
            if commit.is_empty()
                || commit.len() > 256
                || !commit.bytes().all(|byte| byte.is_ascii_hexdigit())
                || path.is_empty()
                || path.len() > 4096
                || path.starts_with('/')
                || path.contains('\\')
                || path
                    .split('/')
                    .any(|part| part.is_empty() || part == "." || part == "..")
                || path.chars().any(char::is_control)
                || end <= start
                || end - start > 80
                || *end > 1_000_000
            {
                return Err(invalid());
            }
        }
        HistoryIndexCommand::Start {} | HistoryIndexCommand::Status {} => {}
    }
    Ok(())
}

fn expression(value: &HistoryExpression, depth: usize, nodes: &mut usize) -> Result<()> {
    *nodes += 1;
    if depth > 16 || *nodes > 128 {
        return Err(invalid());
    }
    match value {
        HistoryExpression::Term { field, value } => {
            if value.is_empty()
                || value.len() > 4096
                || !matches!(
                    field.as_deref(),
                    None | Some(
                        "content" | "path" | "file" | "author" | "commit" | "after" | "before"
                    )
                )
                || (matches!(field.as_deref(), Some("after" | "before"))
                    && value.parse::<i64>().is_err())
            {
                return Err(invalid());
            }
        }
        HistoryExpression::And { clauses } | HistoryExpression::Or { clauses } => {
            for clause in clauses {
                expression(clause, depth + 1, nodes)?;
            }
        }
        HistoryExpression::Not { operand } => expression(operand, depth + 1, nodes)?,
    }
    Ok(())
}
