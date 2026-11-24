use super::CredentialKey;
use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use lapis_app_core::credentials::MAX_SECRET_BYTES;
use std::io;
use uuid::Uuid;
use zeroize::Zeroizing;

pub(super) struct Cipher {
    key: Zeroizing<[u8; 32]>,
    installation: Uuid,
}
pub(super) fn unavailable() -> io::Error {
    io::Error::other("Repository credential storage is locked or unavailable")
}
impl Cipher {
    pub fn installation(&self) -> Uuid {
        self.installation
    }
    pub fn new(key: &CredentialKey, installation: Uuid) -> io::Result<Self> {
        if installation.is_nil() {
            return Err(unavailable());
        }
        Ok(Self {
            key: Zeroizing::new(*key.as_bytes()),
            installation,
        })
    }
    pub(super) fn aad(&self, owner: Uuid, id: Uuid) -> Vec<u8> {
        let mut result = b"lapis.repository.credentials.v1\0".to_vec();
        result.extend_from_slice(self.installation.as_bytes());
        result.extend_from_slice(owner.as_bytes());
        result.extend_from_slice(id.as_bytes());
        result
    }
    pub fn seal(&self, owner: Uuid, id: Uuid, plaintext: &[u8]) -> io::Result<Vec<u8>> {
        self.seal_aad(&self.aad(owner, id), plaintext)
    }
    pub(super) fn seal_aad(&self, aad: &[u8], plaintext: &[u8]) -> io::Result<Vec<u8>> {
        let mut nonce = [0u8; 12];
        getrandom::fill(&mut nonce).map_err(|_| unavailable())?;
        let ciphertext = Aes256Gcm::new_from_slice(self.key.as_ref())
            .map_err(|_| unavailable())?
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: plaintext,
                    aad,
                },
            )
            .map_err(|_| unavailable())?;
        let mut result = vec![1];
        result.extend_from_slice(&nonce);
        result.extend_from_slice(&ciphertext);
        Ok(result)
    }
    pub fn open(&self, owner: Uuid, id: Uuid, bytes: &[u8]) -> io::Result<Zeroizing<Vec<u8>>> {
        self.open_aad(&self.aad(owner, id), bytes)
    }
    pub(super) fn open_aad(&self, aad: &[u8], bytes: &[u8]) -> io::Result<Zeroizing<Vec<u8>>> {
        if bytes.len() < 29 || bytes[0] != 1 || bytes.len() > MAX_SECRET_BYTES + 29 {
            return Err(unavailable());
        }
        let plaintext = Aes256Gcm::new_from_slice(self.key.as_ref())
            .map_err(|_| unavailable())?
            .decrypt(
                Nonce::from_slice(&bytes[1..13]),
                Payload {
                    msg: &bytes[13..],
                    aad,
                },
            )
            .map_err(|_| unavailable())?;
        Ok(Zeroizing::new(plaintext))
    }
}
