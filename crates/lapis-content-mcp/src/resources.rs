use lapis_app_protocol::content::ContentCommand;
use percent_encoding::percent_decode_str;
use rmcp::ErrorData;
use uuid::Uuid;

pub const TEMPLATE: &str = "lapis://notes/{vault_id}/{path}";

/// Parse the literal URI without URL path normalization. Decode exactly once;
/// Core owns path validation, vault authorization and saved-byte access.
pub fn command(uri: &str) -> Result<ContentCommand, ErrorData> {
    let invalid = || ErrorData::invalid_params("Invalid Lapis note resource URI", None);
    if uri.len() > 16 * 1024 || uri.contains(['?', '#']) {
        return Err(invalid());
    }
    let (vault, path) = uri
        .strip_prefix("lapis://notes/")
        .and_then(|rest| rest.split_once('/'))
        .ok_or_else(invalid)?;
    let vault_id = Uuid::parse_str(vault).map_err(|_| invalid())?;
    let bytes = path.as_bytes();
    for (index, &byte) in bytes.iter().enumerate() {
        if byte == b'%'
            && !bytes
                .get(index + 1..index + 3)
                .is_some_and(|pair| pair.iter().all(u8::is_ascii_hexdigit))
        {
            return Err(invalid());
        }
    }
    let path = percent_decode_str(path)
        .decode_utf8()
        .map_err(|_| invalid())?
        .into_owned();
    Ok(ContentCommand::ReadNote { vault_id, path })
}
