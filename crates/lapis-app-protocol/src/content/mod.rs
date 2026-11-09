//! Portable content automation contracts. This catalog does not enable host capabilities.
//! Adapters must validate requests before admission; Core retains authorization,
//! physical path checks, writer fencing and execution. Browser protocol v3 is unchanged.
mod access;
mod catalog;
mod request;
mod response;
mod validation;

pub use access::*;
pub use catalog::*;
pub use request::*;
pub use response::*;
pub use validation::validate_content_path;

pub const CONTENT_API_MAJOR: u32 = 1;
pub const MAX_CONTENT_PAGE_SIZE: u16 = 100;
pub const MAX_CONTENT_TEXT_BYTES: usize = 1024 * 1024;
pub const MAX_CONTENT_PATH_BYTES: usize = 4096;
pub const MAX_CONTENT_CURSOR_BYTES: usize = 4096;
pub const MAX_CONTENT_VERSION_BYTES: usize = 256;
