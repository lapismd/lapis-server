use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use std::io;
use subtle::ConstantTimeEq;

fn parameters() -> scrypt::Params {
    scrypt::Params::new(17, 8, 1, 64).expect("fixed bounded scrypt parameters")
}
fn decoded(encoded: &str) -> Option<(Vec<u8>, Vec<u8>)> {
    let parts: Vec<_> = encoded.split('$').collect();
    if parts.len() != 6 || parts[..4] != ["scrypt", "131072", "8", "1"] {
        return None;
    }
    let salt = URL_SAFE_NO_PAD.decode(parts[4]).ok()?;
    let expected = URL_SAFE_NO_PAD.decode(parts[5]).ok()?;
    (salt.len() == 16 && expected.len() == 64).then_some((salt, expected))
}
pub fn valid(encoded: &str) -> bool {
    decoded(encoded).is_some()
}
pub fn hash(password: &str) -> io::Result<String> {
    if !(12..=1024).contains(&password.encode_utf16().count()) {
        return Err(io::Error::other(
            "Password must contain between 12 and 1024 characters",
        ));
    }
    let mut salt = [0; 16];
    getrandom::fill(&mut salt).map_err(|_| io::Error::other("Password entropy is unavailable"))?;
    let mut output = [0; 64];
    scrypt::scrypt(password.as_bytes(), &salt, &parameters(), &mut output)
        .map_err(|_| io::Error::other("Password derivation failed"))?;
    let salt = URL_SAFE_NO_PAD.encode(salt);
    let output = URL_SAFE_NO_PAD.encode(output);
    Ok(format!("scrypt$131072$8$1${salt}${output}"))
}
pub fn verify(password: &str, encoded: &str) -> bool {
    if password.encode_utf16().count() > 1024 {
        return false;
    }
    let Some((salt, expected)) = decoded(encoded) else {
        return false;
    };
    let mut output = [0; 64];
    scrypt::scrypt(password.as_bytes(), &salt, &parameters(), &mut output).is_ok()
        && bool::from(output.as_slice().ct_eq(&expected))
}
pub(super) fn token() -> io::Result<String> {
    let mut bytes = [0; 32];
    getrandom::fill(&mut bytes).map_err(|_| io::Error::other("Session entropy is unavailable"))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
pub(super) fn digest(value: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
