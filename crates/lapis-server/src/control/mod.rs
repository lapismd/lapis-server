mod attempts;
pub mod bootstrap;
mod login;
mod owner;
pub mod passwords;
mod sessions;
pub(crate) mod vaults;

pub use login::LoginError;
pub use sessions::{IssuedSession, Session};
use std::{io, path::Path, sync::Arc};
use tokio::sync::{Mutex, Semaphore};

pub const SESSION_IDLE_MS: i64 = 7 * 86_400_000;
pub const SESSION_ABSOLUTE_MS: i64 = 30 * 86_400_000;

pub struct Control {
    _database: turso::Database,
    connection: Mutex<Option<turso::Connection>>,
    login_slot: Arc<Semaphore>,
}
impl Control {
    pub async fn open(profile: &Path) -> io::Result<Self> {
        if !profile.is_absolute() {
            return Err(io::Error::other("Control profile must be absolute"));
        }
        let directory = profile.join("control");
        std::fs::create_dir_all(&directory)?;
        if std::fs::symlink_metadata(&directory)?
            .file_type()
            .is_symlink()
        {
            return Err(io::Error::other("Control directory cannot be a symlink"));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))?;
        }
        let path = directory.join("auth.turso");
        if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(io::Error::other("Control database cannot be a symlink"));
        }
        let database = turso::Builder::new_local(path.to_str().ok_or_else(unavailable)?)
            .experimental_attach(false)
            .experimental_vacuum(false)
            .build()
            .await
            .map_err(engine)?;
        let connection = database.connect().map_err(engine)?;
        connection
            .execute_batch(include_str!("schema.sql"))
            .await
            .map_err(engine)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }
        Ok(Self {
            _database: database,
            connection: Mutex::new(Some(connection)),
            login_slot: Arc::new(Semaphore::new(1)),
        })
    }
    pub async fn close(&self) -> io::Result<()> {
        let mut owned = self.connection.lock().await;
        if let Some(connection) = owned.as_ref() {
            let mut rows = connection
                .query("PRAGMA wal_checkpoint(TRUNCATE)", ())
                .await
                .map_err(engine)?;
            while let Some(row) = rows.next().await.map_err(engine)? {
                if row.get::<i64>(0).map_err(engine)? != 0 {
                    return Err(io::Error::other("Control checkpoint is busy"));
                }
            }
        }
        *owned = None;
        Ok(())
    }
}
fn engine(_: turso::Error) -> io::Error {
    unavailable()
}
fn unavailable() -> io::Error {
    io::Error::other("Control database is unavailable")
}
fn validate_time(now: i64) -> io::Result<()> {
    if !(0..=9_007_199_254_740_991 - SESSION_ABSOLUTE_MS).contains(&now) {
        return Err(io::Error::other("Invalid authentication timestamp"));
    }
    Ok(())
}
pub fn now_ms() -> io::Result<i64> {
    let value = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| unavailable())?
        .as_millis();
    let now = i64::try_from(value).map_err(|_| unavailable())?;
    validate_time(now)?;
    Ok(now)
}
