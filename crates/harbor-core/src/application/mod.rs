//! The **application** layer: use-case orchestration.
//!
//! Services here depend only on the [`domain`](crate::domain) layer and on the
//! *ports* defined in [`ports`]. Concrete adapters are injected as
//! `Arc<dyn Trait>` so the services can be unit tested against in-memory fakes.

pub mod ports;

mod key_service;
pub mod metrics_service;
mod profile_service;
mod session_service;
pub mod transfer_service;

pub use key_service::KeyService;
pub use metrics_service::{parse_metrics, MetricsService, METRICS_COMMAND};
pub use profile_service::ProfileService;
pub use session_service::{SessionEvent, SessionService};
pub use transfer_service::{SftpProvider, TransferEvent, TransferRequest, TransferService};
