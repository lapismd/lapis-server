use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::{io::Read, path::Path};
use zeroize::Zeroizing;

pub const KEY_ENV: &str = "LAPIS_SERVER_CREDENTIAL_KEY";
pub const KEY_FILE_ENV: &str = "LAPIS_SERVER_CREDENTIAL_KEY_FILE";

/// Startup-only master key. This type deliberately has no formatting implementation.
pub struct CredentialKey(Zeroizing<[u8; 32]>);

impl CredentialKey {
    pub fn from_base64(value: &[u8]) -> Option<Self> {
        if value.len() != 44 {
            return None;
        }
        let mut decoded = Zeroizing::new([0; 33]);
        if STANDARD.decode_slice(value, &mut decoded[..]).ok()? != 32 {
            return None;
        }
        Self::read(&decoded[..32])
    }

    pub fn from_file(path: &Path) -> Option<Self> {
        if !path.is_absolute() || !std::fs::metadata(path).ok()?.is_file() {
            return None;
        }
        let file = std::fs::File::open(path).ok()?;
        let metadata = file.metadata().ok()?;
        if !metadata.is_file() || metadata.len() != 32 {
            return None;
        }
        Self::read(file)
    }

    pub fn read(mut reader: impl Read) -> Option<Self> {
        let mut key = Zeroizing::new([0; 32]);
        reader.read_exact(&mut key[..]).ok()?;
        let mut extra = Zeroizing::new([0]);
        (reader.read(&mut extra[..]).ok()? == 0).then_some(Self(key))
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_and_base64_inputs_decode_to_the_same_key() {
        let raw = [37u8; 32];
        let value = STANDARD.encode(raw);
        assert_eq!(
            CredentialKey::from_base64(value.as_bytes())
                .unwrap()
                .as_bytes(),
            &raw
        );
        assert_eq!(CredentialKey::read(&raw[..]).unwrap().as_bytes(), &raw);
        assert!(CredentialKey::from_base64(value.trim_end_matches('=').as_bytes()).is_none());
        assert!(CredentialKey::from_base64(&[b'!'; 44]).is_none());
        assert!(CredentialKey::read(&raw[..31]).is_none());
        assert!(CredentialKey::read(&[0; 33][..]).is_none());
    }
}
