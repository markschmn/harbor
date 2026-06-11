//! Terminal session value objects.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::profile::ProfileId;

/// Stable identifier for a live terminal/SSH session (one per tab).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(pub Uuid);

impl SessionId {
    pub fn new() -> Self {
        SessionId(Uuid::new_v4())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Lifecycle state of a session, surfaced to the UI for status indicators and
/// reconnect handling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum SessionStatus {
    Connecting,
    Connected,
    Disconnected { reason: String },
    Failed { reason: String },
}

/// Desired pseudo-terminal geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PtySize {
    pub cols: u32,
    pub rows: u32,
    #[serde(default)]
    pub pixel_width: u32,
    #[serde(default)]
    pub pixel_height: u32,
}

impl Default for PtySize {
    fn default() -> Self {
        PtySize {
            cols: 80,
            rows: 24,
            pixel_width: 0,
            pixel_height: 0,
        }
    }
}

/// Snapshot describing a session, sent to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: SessionId,
    pub profile_id: Option<ProfileId>,
    pub title: String,
    pub status: SessionStatus,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_pty_is_80x24() {
        let s = PtySize::default();
        assert_eq!((s.cols, s.rows), (80, 24));
    }

    #[test]
    fn status_serialises_with_tag() {
        let s = SessionStatus::Disconnected {
            reason: "network".into(),
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("\"state\":\"disconnected\""));
        assert!(json.contains("network"));
    }
}
