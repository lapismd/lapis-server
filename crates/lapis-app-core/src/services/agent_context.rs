use super::{Error, Result, Work};
use crate::context::{AttachmentContext, prepare_attachments};
use lapis_app_protocol::ErrorCode;
use std::io::{self, Read};

impl Work {
    /// Read explicit references using this accepted work's admitted vault.
    pub fn attachment_context(
        &self,
        paths: &[String],
        cancelled: impl Fn() -> bool,
    ) -> Result<AttachmentContext> {
        prepare_attachments(
            paths,
            |path| self.context_file(path),
            || cancelled() || self.cancellation_requested(),
        )
    }

    fn context_file(&self, path: &str) -> Result<Option<String>> {
        let state = self.vault.lock()?;
        state.root.verify_physical_root()?;
        let file = match state.root.open_file(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        let mut bytes = Vec::new();
        file.take(65_537).read_to_end(&mut bytes)?;
        state.root.verify_physical_root()?;
        if bytes.len() > 65_536 {
            return Err(Error::new(
                ErrorCode::InvalidRequest,
                "File context exceeds 64 KiB per file or 128 KiB per message",
            ));
        }
        String::from_utf8(bytes)
            .map(Some)
            .map_err(|_| Error::new(ErrorCode::InvalidRequest, "File is not valid UTF-8"))
    }
}
