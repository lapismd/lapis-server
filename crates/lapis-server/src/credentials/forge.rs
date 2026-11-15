//! Host-only encrypted backing for the forge manager's opaque credential JSON.
use super::{CredentialStore, Unlocked, unavailable};
use std::{io, sync::atomic::Ordering};
use uuid::Uuid;
use zeroize::Zeroizing;

const LIMIT: usize = 128;
const MAX_BYTES: usize = 12 * 1024;
fn identity(owner: Uuid, account: &str) -> io::Result<()> {
    if owner.is_nil()
        || account.len() != 64
        || !account
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(unavailable());
    }
    Ok(())
}
fn path(owner: Uuid, account: &str) -> String {
    format!(".forge/{owner}-{account}.sealed")
}
impl CredentialStore {
    pub fn forge_get(&self, owner: Uuid, account: &str) -> io::Result<Option<Zeroizing<String>>> {
        identity(owner, account)?;
        let (store, _guard) = self.ready()?;
        store
            .forge_read(owner, account)
            .map(|value| value.map(|v| v.0))
    }
    pub fn forge_set(
        &self,
        owner: Uuid,
        account: &str,
        value: Zeroizing<String>,
    ) -> io::Result<()> {
        identity(owner, account)?;
        if value.is_empty() || value.len() > MAX_BYTES {
            return Err(unavailable());
        }
        let (store, _guard) = self.ready()?;
        if let Err(e) = store.root.mkdir(".forge", false)
            && e.kind() != io::ErrorKind::AlreadyExists
        {
            return Err(unavailable());
        }
        store.forge_directory()?;
        let entries = store.root.open_directory(".forge")?;
        let mut count = 0;
        for entry in entries.entries()? {
            let entry = entry?;
            let name = entry.file_name().into_string().map_err(|_| unavailable())?;
            if name.starts_with(".lapis-stage-") {
                continue;
            }
            let stem = name.strip_suffix(".sealed").ok_or_else(unavailable)?;
            if stem.len() != 101 || !stem.is_ascii() || !entry.file_type()?.is_file() {
                return Err(unavailable());
            }
            let found_owner = Uuid::parse_str(&stem[..36]).map_err(|_| unavailable())?;
            identity(found_owner, &stem[37..])?;
            if name != format!("{found_owner}-{}.sealed", &stem[37..]) {
                return Err(unavailable());
            }
            count += 1;
            if count > LIMIT {
                return Err(unavailable());
            }
        }
        let before = store.forge_read(owner, account)?;
        if before.is_none() && count >= LIMIT {
            return Err(unavailable());
        }
        let encrypted = store.cipher.seal_forge(owner, account, value.as_bytes())?;
        let name = path(owner, account);
        store.root.write_if_unchanged(
            &name,
            &encrypted[..],
            before.as_ref().map(|v| v.1.as_slice()),
            &Default::default(),
        )?;
        store.verify()
    }
    pub fn forge_remove(&self, owner: Uuid, account: &str) -> io::Result<()> {
        identity(owner, account)?;
        let (store, _guard) = self.ready()?;
        if store.forge_read(owner, account)?.is_some() {
            store.root.remove_file(&path(owner, account))?;
        }
        store.verify()
    }
}
impl Unlocked {
    pub(super) fn has_forge_records(&self) -> io::Result<bool> {
        if !self.forge_directory()? {
            return Ok(false);
        }
        Ok(self
            .root
            .open_directory(".forge")?
            .entries()?
            .next()
            .is_some())
    }
    fn forge_directory(&self) -> io::Result<bool> {
        self.verify()?;
        match self.root.entry_metadata(".forge") {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => Ok(true),
            _ => Err(unavailable()),
        }
    }
    fn forge_read(
        &self,
        owner: Uuid,
        account: &str,
    ) -> io::Result<Option<(Zeroizing<String>, Vec<u8>)>> {
        if !self.forge_directory()? {
            return Ok(None);
        }
        let name = path(owner, account);
        match self.root.entry_metadata(&name) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(unavailable()),
            Ok(_) => {}
        }
        let encrypted = self.bytes(&name)?;
        let plain = self
            .cipher
            .open_forge(owner, account, &encrypted)
            .inspect_err(|_| {
                self.locked.store(true, Ordering::Release);
            })?;
        if plain.is_empty() || plain.len() > MAX_BYTES {
            return Err(unavailable());
        }
        let value = std::str::from_utf8(&plain).map_err(|_| unavailable())?;
        Ok(Some((Zeroizing::new(value.to_owned()), encrypted)))
    }
}
