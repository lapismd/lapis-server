//! Bounded content archives; host catalogue and publication policy stay outside.
mod export;
mod import;

pub use export::create_zip;
pub use import::extract_zip;
use std::io;

#[derive(Clone, Copy, Debug)]
pub struct ArchiveLimits {
    pub archive_bytes: u64,
    pub expanded_bytes: u64,
    pub entries: usize,
}
impl Default for ArchiveLimits {
    fn default() -> Self {
        Self {
            archive_bytes: 512 * 1024 * 1024,
            expanded_bytes: 2 * 1024 * 1024 * 1024,
            entries: 100_000,
        }
    }
}
pub struct ArchiveSnapshot {
    pub file: std::fs::File,
    pub length: u64,
}
fn invalid(message: impl ToString) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.to_string())
}
fn zip_error(_: zip::result::ZipError) -> io::Error {
    invalid("Invalid or unsupported ZIP archive")
}
