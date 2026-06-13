//! The **domain** layer: pure entities, value objects and invariants.
//!
//! Nothing in this module performs I/O, talks to the network or touches the
//! filesystem. Everything here is deterministic and trivially testable.

pub mod auth;
pub mod error;
pub mod file;
pub mod host_key;
pub mod key;
pub mod metrics;
pub mod profile;
pub mod session;
pub mod transfer;
