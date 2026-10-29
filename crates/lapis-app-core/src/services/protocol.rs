use super::{Client, Error};
use lapis_app_protocol::{
    ErrorCode, Outcome, PROTOCOL_MAJOR, Reply, Request, VaultCommand, VaultValue,
};

impl Client {
    pub fn execute(&self, request: Request) -> Reply {
        let result = (|| {
            if request.protocol_version != PROTOCOL_MAJOR {
                return Err(Error::new(
                    ErrorCode::ProtocolMismatch,
                    "Reload the matching application client",
                ));
            }
            if request.request_id.is_nil() {
                return Err(Error::new(
                    ErrorCode::InvalidRequest,
                    "Request identity is required",
                ));
            }
            if request.vault_id != self.vault.id || request.generation != self.vault.generation {
                return Err(Error::denied());
            }
            Ok(match request.command {
                VaultCommand::EditingIntents { command } => VaultValue::EditingIntents {
                    value: self.editing_intents(command)?,
                },
                VaultCommand::Files { operation } => VaultValue::Files {
                    value: self.files(operation)?,
                },
                VaultCommand::ReadText { path } => VaultValue::Text {
                    snapshot: self.read_text(&path)?,
                },
                VaultCommand::WriteText {
                    path,
                    text,
                    expected,
                } => VaultValue::Written {
                    version: self.write_text(&path, &text, &expected)?,
                },
                VaultCommand::OpenEditor { path } => VaultValue::Editor {
                    editor_id: self.open_editor(&path)?,
                },
                VaultCommand::MarkDirty {
                    editor_id,
                    revision,
                } => {
                    self.mark_dirty(editor_id, revision)?;
                    VaultValue::Done {}
                }
                VaultCommand::MarkSaved {
                    editor_id,
                    revision,
                    version,
                } => VaultValue::Saved {
                    cleared: self.mark_saved(editor_id, revision, &version)?,
                },
                VaultCommand::CloseEditor { editor_id } => {
                    self.close_editor(editor_id)?;
                    VaultValue::Done {}
                }
                VaultCommand::DiscardEditor { editor_id } => {
                    self.discard_editor(editor_id)?;
                    VaultValue::Done {}
                }
                VaultCommand::RecoverEditor { editor_id } => {
                    self.recover_editor(editor_id)?;
                    VaultValue::Done {}
                }
                VaultCommand::EditorClaims {} => VaultValue::EditorClaims {
                    claims: self.editor_claims()?,
                },
                VaultCommand::Status {} => {
                    self.vault.lock()?.require_client(self.id)?;
                    VaultValue::Status {
                        status: self.vault.status()?,
                    }
                }
            })
        })();
        Reply {
            protocol_version: PROTOCOL_MAJOR,
            request_id: request.request_id,
            outcome: match result {
                Ok(value) => Outcome::Ok { value },
                Err(error) => Outcome::Error { error: error.0 },
            },
        }
    }
}
