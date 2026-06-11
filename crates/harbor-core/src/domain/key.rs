//! Value objects describing discovered SSH keys.

use serde::{Deserialize, Serialize};

/// A private/public key pair discovered on disk (typically under `~/.ssh`).
///
/// Harbor never parses or stores private key *material* in this struct — only
/// metadata derived from the public side, plus whether the private key is
/// encrypted. This keeps secret bytes out of the application state entirely.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscoveredKey {
    /// Absolute path to the private key file.
    pub private_key_path: String,
    /// Absolute path to the matching `.pub` file, if one exists.
    pub public_key_path: Option<String>,
    /// Key algorithm, e.g. `ssh-ed25519`, `ssh-rsa`, `ecdsa-sha2-nistp256`.
    pub algorithm: String,
    /// SHA256 fingerprint in OpenSSH format, e.g. `SHA256:abc...`.
    pub fingerprint: String,
    /// The comment field of the public key (often `user@host`).
    pub comment: Option<String>,
    /// Bit size where meaningful (RSA); `None` for fixed-size algorithms.
    pub bits: Option<usize>,
    /// Whether the private key is passphrase-encrypted.
    pub encrypted: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovered_key_round_trips_json() {
        let k = DiscoveredKey {
            private_key_path: "/home/me/.ssh/id_ed25519".into(),
            public_key_path: Some("/home/me/.ssh/id_ed25519.pub".into()),
            algorithm: "ssh-ed25519".into(),
            fingerprint: "SHA256:abcdef".into(),
            comment: Some("me@laptop".into()),
            bits: None,
            encrypted: true,
        };
        let s = serde_json::to_string(&k).unwrap();
        let parsed: DiscoveredKey = serde_json::from_str(&s).unwrap();
        assert_eq!(k, parsed);
    }
}
