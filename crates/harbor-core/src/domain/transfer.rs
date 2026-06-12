//! File transfer value objects used by the transfer manager.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use super::session::SessionId;

/// Stable identifier for a transfer task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TransferId(pub Uuid);

impl TransferId {
    pub fn new() -> Self {
        TransferId(Uuid::new_v4())
    }
}

impl Default for TransferId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TransferId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Whether a transfer uploads to or downloads from the remote host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferDirection {
    Upload,
    Download,
}

/// Lifecycle of a transfer task.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum TransferState {
    Queued,
    Active,
    Paused,
    Completed,
    Failed { error: String },
    Cancelled,
}

impl TransferState {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TransferState::Completed | TransferState::Failed { .. } | TransferState::Cancelled
        )
    }

    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            TransferState::Failed { .. } | TransferState::Cancelled
        )
    }
}

/// A single queued/active/finished transfer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferTask {
    pub id: TransferId,
    /// The SSH session this transfer runs over.
    pub session_id: SessionId,
    pub direction: TransferDirection,
    /// Source path (local for upload, remote for download).
    pub source: String,
    /// Destination path (remote for upload, local for download).
    pub destination: String,
    /// Display name (the file's base name).
    pub file_name: String,
    pub state: TransferState,
    /// Total size in bytes, if known up-front.
    pub total_bytes: u64,
    /// Bytes transferred so far.
    pub transferred_bytes: u64,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub started_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub finished_at: Option<OffsetDateTime>,
}

impl TransferTask {
    /// Completion ratio in `0.0..=1.0`. Returns `1.0` for completed zero-byte
    /// files and `0.0` when the size is unknown.
    pub fn progress(&self) -> f64 {
        if matches!(self.state, TransferState::Completed) {
            return 1.0;
        }
        if self.total_bytes == 0 {
            return 0.0;
        }
        (self.transferred_bytes as f64 / self.total_bytes as f64).clamp(0.0, 1.0)
    }
}

/// A lightweight progress event streamed to the UI while a transfer runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferProgress {
    pub id: TransferId,
    pub transferred_bytes: u64,
    pub total_bytes: u64,
    /// Instantaneous throughput in bytes/second, smoothed by the caller.
    pub bytes_per_second: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_is_clamped_and_handles_unknown_size() {
        let mut t = TransferTask {
            id: TransferId::new(),
            session_id: SessionId::new(),
            direction: TransferDirection::Upload,
            source: "/a".into(),
            destination: "/b".into(),
            file_name: "a".into(),
            state: TransferState::Active,
            total_bytes: 0,
            transferred_bytes: 50,
            created_at: OffsetDateTime::UNIX_EPOCH,
            started_at: None,
            finished_at: None,
        };
        assert_eq!(t.progress(), 0.0); // unknown size

        t.total_bytes = 100;
        t.transferred_bytes = 50;
        assert_eq!(t.progress(), 0.5);

        t.transferred_bytes = 999; // never exceed 1.0
        assert_eq!(t.progress(), 1.0);

        t.state = TransferState::Completed;
        t.total_bytes = 0;
        assert_eq!(t.progress(), 1.0);
    }

    #[test]
    fn terminal_and_retryable_states() {
        assert!(TransferState::Completed.is_terminal());
        assert!(!TransferState::Active.is_terminal());
        assert!(TransferState::Failed { error: "x".into() }.is_retryable());
        assert!(TransferState::Cancelled.is_retryable());
        assert!(!TransferState::Completed.is_retryable());
    }
}
