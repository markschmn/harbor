//! The **presentation** layer: Tauri commands exposed to the React frontend.
//!
//! Each command is a thin adapter that parses arguments, delegates to a
//! `harbor-core` application service, and maps errors to [`CommandError`]
//! (`crate::error`). No business logic lives here.

pub mod app;
pub mod keys;
pub mod local_fs;
pub mod profiles;
pub mod security;
pub mod sessions;
pub mod sftp;
pub mod transfers;
