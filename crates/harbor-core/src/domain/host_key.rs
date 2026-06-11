//! Host key value objects and the verification decision model.
//!
//! Harbor follows OpenSSH conventions: a server is identified by its public
//! host key, recorded in a `known_hosts` file on first connection (Trust On
//! First Use). On every subsequent connection the presented key must match the
//! stored one, otherwise the connection is aborted — a changed key may indicate
//! a man-in-the-middle attack.

use serde::{Deserialize, Serialize};

/// A server host key, stored in the OpenSSH base64 wire format together with
/// its algorithm name (e.g. `ssh-ed25519`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostKey {
    /// Algorithm name as it appears in `known_hosts`, e.g. `ssh-ed25519`,
    /// `ecdsa-sha2-nistp256`, `ssh-rsa`.
    pub algorithm: String,
    /// Base64-encoded public key blob (no algorithm prefix, no comment).
    pub key_base64: String,
}

impl HostKey {
    pub fn new(algorithm: impl Into<String>, key_base64: impl Into<String>) -> Self {
        HostKey {
            algorithm: algorithm.into(),
            key_base64: key_base64.into(),
        }
    }

    /// The `known_hosts` public-key field: `<algorithm> <base64>`.
    pub fn authorized_key(&self) -> String {
        format!("{} {}", self.algorithm, self.key_base64)
    }
}

/// A single parsed line from a `known_hosts` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownHostEntry {
    /// The host pattern field. May be a plain `host`, `host:port` style
    /// `[host]:port`, a comma separated list, or a hashed `|1|salt|hash` token.
    pub host_field: String,
    /// The key carried on this line.
    pub key: HostKey,
    /// `true` when the host field is hashed (`|1|...`). Hashed entries protect
    /// the list of hosts a user has connected to.
    pub hashed: bool,
    /// A `@revoked` or `@cert-authority` marker, if present.
    pub marker: Option<String>,
}

/// The outcome of comparing a presented host key against `known_hosts`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostKeyDecision {
    /// The host is known and the presented key matches a trusted entry.
    Trusted,
    /// The host has never been seen before. The caller must ask the user
    /// whether to trust and persist this key (TOFU). The fingerprint is
    /// provided so it can be shown for out-of-band verification.
    Unknown { fingerprint: String },
    /// The host is known but presented a *different* key. This is a hard error
    /// and the connection must be refused.
    Mismatch {
        fingerprint: String,
        expected_algorithms: Vec<String>,
    },
    /// The presented key is explicitly revoked (`@revoked`). Always refuse.
    Revoked { fingerprint: String },
}

impl HostKeyDecision {
    /// Whether a connection may proceed *without* user interaction.
    pub fn is_trusted(&self) -> bool {
        matches!(self, HostKeyDecision::Trusted)
    }

    /// Whether this decision represents a security failure that must abort the
    /// connection regardless of user preference.
    pub fn is_hard_failure(&self) -> bool {
        matches!(
            self,
            HostKeyDecision::Mismatch { .. } | HostKeyDecision::Revoked { .. }
        )
    }
}

/// How the user resolved an [`HostKeyDecision::Unknown`] prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TofuResolution {
    /// Trust this key and persist it to `known_hosts` for future connections.
    TrustAndSave,
    /// Trust for this session only; do not persist.
    TrustOnce,
    /// Refuse the connection.
    Reject,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorized_key_formats_correctly() {
        let k = HostKey::new("ssh-ed25519", "AAAAC3NzaC1lZDI1NTE5");
        assert_eq!(k.authorized_key(), "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5");
    }

    #[test]
    fn decision_classification() {
        assert!(HostKeyDecision::Trusted.is_trusted());
        assert!(!HostKeyDecision::Trusted.is_hard_failure());

        let mismatch = HostKeyDecision::Mismatch {
            fingerprint: "SHA256:abc".into(),
            expected_algorithms: vec!["ssh-ed25519".into()],
        };
        assert!(mismatch.is_hard_failure());
        assert!(!mismatch.is_trusted());

        let revoked = HostKeyDecision::Revoked {
            fingerprint: "SHA256:abc".into(),
        };
        assert!(revoked.is_hard_failure());
    }
}
