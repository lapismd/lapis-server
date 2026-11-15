use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, Payload},
};
use std::{
    io::{self, Read},
    path::Path,
};
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
    pub fn load(path: &Path, installation: Uuid) -> io::Result<Self> {
        if installation.is_nil() || !path.is_absolute() {
            return Err(unavailable());
        }
        let metadata = std::fs::metadata(path).map_err(|_| unavailable())?;
        if !metadata.is_file() || metadata.len() != 32 {
            return Err(unavailable());
        }
        let mut bytes = Zeroizing::new(Vec::new());
        std::fs::File::open(path)
            .map_err(|_| unavailable())?
            .take(33)
            .read_to_end(&mut bytes)
            .map_err(|_| unavailable())?;
        if bytes.len() != 32 {
            return Err(unavailable());
        }
        let mut key = Zeroizing::new([0u8; 32]);
        key.copy_from_slice(&bytes);
        Ok(Self { key, installation })
    }
    fn aad(&self, owner: Uuid, id: Uuid) -> Vec<u8> {
        let mut result = b"lapis.repository.credentials.v1\0".to_vec();
        result.extend_from_slice(self.installation.as_bytes());
        result.extend_from_slice(owner.as_bytes());
        result.extend_from_slice(id.as_bytes());
        result
    }
    pub fn seal(&self, owner: Uuid, id: Uuid, plaintext: &[u8]) -> io::Result<Vec<u8>> {
        let mut nonce = [0u8; 12];
        getrandom::fill(&mut nonce).map_err(|_| unavailable())?;
        let ciphertext = Aes256Gcm::new_from_slice(self.key.as_ref())
            .map_err(|_| unavailable())?
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: plaintext,
                    aad: &self.aad(owner, id),
                },
            )
            .map_err(|_| unavailable())?;
        let mut result = vec![1];
        result.extend_from_slice(&nonce);
        result.extend_from_slice(&ciphertext);
        Ok(result)
    }
    pub fn open(&self, owner: Uuid, id: Uuid, bytes: &[u8]) -> io::Result<Zeroizing<Vec<u8>>> {
        if bytes.len() < 29 || bytes[0] != 1 || bytes.len() > 16 * 1024 {
            return Err(unavailable());
        }
        let plaintext = Aes256Gcm::new_from_slice(self.key.as_ref())
            .map_err(|_| unavailable())?
            .decrypt(
                Nonce::from_slice(&bytes[1..13]),
                Payload {
                    msg: &bytes[13..],
                    aad: &self.aad(owner, id),
                },
            )
            .map_err(|_| unavailable())?;
        Ok(Zeroizing::new(plaintext))
    }
}
