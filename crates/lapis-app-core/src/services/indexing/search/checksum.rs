use serde_json::json;

/// Retain SearchManager's historical JavaScript code-unit packing, then reuse
/// the existing MD5 primitive. Each fourth unit drops its overflowing high byte;
/// other high bytes OR into the following unit rather than replacing it.
pub(super) fn source(text: &str) -> String {
    let input = json!({"content":text,"metadata":null,"tags":[]}).to_string();
    let mut bytes = vec![0u8; input.encode_utf16().count()];
    for (index, unit) in input.encode_utf16().enumerate() {
        bytes[index] |= unit as u8;
        if index % 4 != 3
            && let Some(next) = bytes.get_mut(index + 1)
        {
            *next |= (unit >> 8) as u8;
        }
    }
    // The serialized envelope ends in ASCII, so no final code-unit overflow
    // touches MD5 padding. The logical byte length remains the UTF-16 length.
    format!("{:x}", md5::compute(bytes))
}
