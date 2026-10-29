#![forbid(unsafe_code)]

//! Native Notes application services shared by Tauri and hosted adapters.

pub mod archive;
pub mod context;
pub mod database;
pub mod embedding;
pub mod plugins;
mod record_store;
pub mod services;
mod text;
pub mod turns;
pub mod vault;
