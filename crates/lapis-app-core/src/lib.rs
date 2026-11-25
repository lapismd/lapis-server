#![forbid(unsafe_code)]

//! Native Notes application services shared by Tauri and hosted adapters.

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod archive;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod context;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod credentials;
#[cfg(any(
    not(any(target_os = "android", target_os = "ios")),
    feature = "mobile-database"
))]
pub mod database;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod embedding;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod plugins;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
mod record_store;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod repository;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod services;
#[cfg(all(
    any(target_os = "android", target_os = "ios"),
    feature = "mobile-database"
))]
#[path = "services/error.rs"]
pub mod services;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod sync;
#[cfg(any(
    not(any(target_os = "android", target_os = "ios")),
    feature = "mobile-database"
))]
mod text;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod turns;
pub mod vault;

#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod skills;
