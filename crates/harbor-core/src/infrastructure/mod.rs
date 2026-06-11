//! The **infrastructure** layer: concrete adapters implementing the
//! application [`ports`](crate::application::ports).
//!
//! Each submodule wires one external concern:
//!
//! * [`storage`] — TOML-backed profile persistence.
//! * [`keychain`] — OS keychain secret storage (with a safe in-memory fallback).
//! * [`known_hosts`] — OpenSSH `known_hosts` parsing, evaluation and writing.
//! * [`keys`] — discovery and fingerprinting of on-disk keys via `ssh-key`.
//! * [`host_key_policy`] — non-interactive host-key prompters for headless use.
//! * [`ssh`] — the russh transport and session.
//! * [`sftp`] — the russh-sftp client adapter.
//! * [`paths`] — resolves Harbor's per-user config/data directories.

pub mod host_key_policy;
pub mod keychain;
pub mod keys;
pub mod known_hosts;
pub mod paths;
pub mod sftp;
pub mod ssh;
pub mod storage;
