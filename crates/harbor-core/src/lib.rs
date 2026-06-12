//! # harbor-core
//!
//! Core library for **Harbor**, a modern, secure SSH client and SFTP file
//! manager. This crate is GUI-agnostic and contains all of the security
//! critical logic so that it can be exhaustively unit tested without a running
//! desktop shell.
//!
//! The crate is organised using a pragmatic *clean architecture* split:
//!
//! * [`domain`] — pure entities, value objects and business rules. No I/O.
//! * [`application`] — use-case orchestration and the *ports* (traits) that the
//!   outer layers must satisfy. Depends only on [`domain`].
//! * [`infrastructure`] — concrete adapters: TOML storage, the OS keychain,
//!   `known_hosts`, SSH/SFTP transport (russh) and key discovery.
//!
//! The presentation layer (the Tauri commands) lives in the separate
//! `src-tauri` crate and depends on this one.

#![forbid(unsafe_code)]
#![warn(missing_debug_implementations)]

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod testing;

pub use domain::error::{HarborError, Result};

/// Convenience re-exports for the most commonly used public types.
pub mod prelude {
    pub use crate::application::ports::{
        KeyDiscovery, KnownHostsStore, ProfileRepository, SecretStore,
    };
    pub use crate::application::{KeyService, ProfileService, SessionService, TransferService};
    pub use crate::domain::auth::{AuthMethod, Credential};
    pub use crate::domain::error::{HarborError, Result};
    pub use crate::domain::host_key::{HostKey, HostKeyDecision, KnownHostEntry};
    pub use crate::domain::profile::{ProfileId, ServerProfile};
    pub use crate::domain::transfer::{TransferDirection, TransferState, TransferTask};
}
