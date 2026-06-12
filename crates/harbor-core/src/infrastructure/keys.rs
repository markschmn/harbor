//! Discovery and inspection of on-disk SSH keys via the `ssh-key` crate.
//!
//! Harbor only ever reads the *public* half plus the encryption flag. Private
//! key material is never decrypted here and never copied into application
//! state — decryption happens transiently at connection time inside the SSH
//! transport.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use ssh_key::{HashAlg, PrivateKey, PublicKey};

use crate::application::ports::KeyDiscovery;
use crate::domain::error::{HarborError, Result};
use crate::domain::key::DiscoveredKey;

use super::paths;

/// Filenames in `~/.ssh` that are never private keys and should be skipped.
const NON_KEY_FILES: &[&str] = &[
    "known_hosts",
    "known_hosts.old",
    "authorized_keys",
    "config",
    "environment",
    "rc",
];

/// Discovers OpenSSH keys under `~/.ssh` (or an explicit directory).
#[derive(Debug, Clone)]
pub struct OpenSshKeyDiscovery {
    dir: PathBuf,
}

impl OpenSshKeyDiscovery {
    /// Scan the user's standard `~/.ssh` directory.
    pub fn with_default_location() -> Result<Self> {
        Ok(Self {
            dir: paths::ssh_dir()?,
        })
    }

    /// Scan an explicit directory (used by tests).
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }
}

/// Inspect a single private key file, deriving metadata from its public half.
pub fn inspect_key_file(path: &Path) -> Result<DiscoveredKey> {
    let text = std::fs::read_to_string(path).map_err(|e| HarborError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;

    let private = PrivateKey::from_openssh(text.as_bytes())
        .map_err(|e| HarborError::Key(format!("{}: {e}", path.display())))?;
    let public = private.public_key();

    let public_key_path = sibling_pub_path(path);
    // The comment of an encrypted key lives in its encrypted section, so prefer
    // the adjacent `.pub` file's comment when available.
    let comment = read_pub_comment(public_key_path.as_deref())
        .or_else(|| non_empty(private.comment().as_str_lossy()))
        .or_else(|| non_empty(public.comment().as_str_lossy()));

    Ok(DiscoveredKey {
        private_key_path: path.to_string_lossy().into_owned(),
        public_key_path: public_key_path.map(|p| p.to_string_lossy().into_owned()),
        algorithm: public.algorithm().as_str().to_string(),
        fingerprint: public.fingerprint(HashAlg::Sha256).to_string(),
        comment,
        bits: key_bits(public),
        encrypted: private.is_encrypted(),
    })
}

fn sibling_pub_path(private: &Path) -> Option<PathBuf> {
    let candidate = PathBuf::from(format!("{}.pub", private.display()));
    candidate.exists().then_some(candidate)
}

fn read_pub_comment(pub_path: Option<&Path>) -> Option<String> {
    let path = pub_path?;
    let text = std::fs::read_to_string(path).ok()?;
    let public = PublicKey::from_openssh(&text).ok()?;
    non_empty(public.comment().as_str_lossy())
}

fn non_empty(s: &str) -> Option<String> {
    let t = s.trim();
    (!t.is_empty()).then(|| t.to_string())
}

/// Best-effort key strength in bits, for display. Only meaningful for RSA/DSA.
fn key_bits(public: &PublicKey) -> Option<usize> {
    use ssh_key::public::KeyData;
    match public.key_data() {
        KeyData::Rsa(rsa) => rsa.n().as_positive_bytes().map(|b| b.len() * 8),
        _ => None,
    }
}

#[async_trait]
impl KeyDiscovery for OpenSshKeyDiscovery {
    async fn discover(&self) -> Result<Vec<DiscoveredKey>> {
        let dir = self.dir.clone();
        tokio::task::spawn_blocking(move || discover_in(&dir))
            .await
            .map_err(|e| HarborError::IoMsg(format!("key discovery task failed: {e}")))?
    }

    async fn inspect(&self, path: &Path) -> Result<DiscoveredKey> {
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || inspect_key_file(&path))
            .await
            .map_err(|e| HarborError::IoMsg(format!("key inspection task failed: {e}")))?
    }
}

fn discover_in(dir: &Path) -> Result<Vec<DiscoveredKey>> {
    let read = match std::fs::read_dir(dir) {
        Ok(r) => r,
        // No `~/.ssh` yet is not an error — just an empty list.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => {
            return Err(HarborError::Io {
                path: dir.to_path_buf(),
                source: e,
            })
        }
    };

    let mut keys = Vec::new();
    for entry in read.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.ends_with(".pub") || NON_KEY_FILES.contains(&name.as_ref()) {
            continue;
        }
        // Attempt to parse; non-keys (e.g. random files) are silently skipped.
        if let Ok(key) = inspect_key_file(&path) {
            keys.push(key);
        }
    }
    Ok(keys)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real OpenSSH key fixtures generated once with `ssh-keygen` and checked
    // into the repository. Using fixtures keeps these tests deterministic and
    // free of any runtime key-generation / RNG dependency.
    const ED25519_PRIV: &str = include_str!("../../tests/fixtures/id_ed25519");
    const ED25519_PUB: &str = include_str!("../../tests/fixtures/id_ed25519.pub");
    const ED25519_ENC_PRIV: &str = include_str!("../../tests/fixtures/id_ed25519_enc");
    const ED25519_ENC_PUB: &str = include_str!("../../tests/fixtures/id_ed25519_enc.pub");

    // Known SHA256 fingerprint of the unencrypted fixture (from `ssh-keygen -lf`).
    const ED25519_FP: &str = "SHA256:1R7eEWgJ9ab7lm32jODAdhB+gegwy/hGp/sBt/sX0wI";

    fn write_pair(dir: &Path, name: &str, priv_pem: &str, pub_line: &str) {
        std::fs::write(dir.join(name), priv_pem).unwrap();
        std::fs::write(dir.join(format!("{name}.pub")), pub_line).unwrap();
    }

    #[test]
    fn inspect_reports_algorithm_fingerprint_and_comment() {
        let dir = tempfile::tempdir().unwrap();
        write_pair(dir.path(), "id_ed25519", ED25519_PRIV, ED25519_PUB);
        let info = inspect_key_file(&dir.path().join("id_ed25519")).unwrap();

        assert_eq!(info.algorithm, "ssh-ed25519");
        assert!(!info.encrypted);
        assert_eq!(info.comment.as_deref(), Some("harbor-test@example"));
        assert!(info.public_key_path.is_some());
        assert_eq!(info.fingerprint, ED25519_FP);
    }

    #[tokio::test]
    async fn discovery_finds_keys_and_skips_non_keys() {
        let dir = tempfile::tempdir().unwrap();
        write_pair(dir.path(), "id_ed25519", ED25519_PRIV, ED25519_PUB);
        write_pair(dir.path(), "id_secure", ED25519_ENC_PRIV, ED25519_ENC_PUB);
        std::fs::write(
            dir.path().join("known_hosts"),
            "example.com ssh-ed25519 AAAA",
        )
        .unwrap();
        std::fs::write(dir.path().join("config"), "Host *\n").unwrap();
        std::fs::write(dir.path().join("random.txt"), "not a key").unwrap();

        let discovery = OpenSshKeyDiscovery::new(dir.path());
        let keys = discovery.discover().await.unwrap();
        assert_eq!(keys.len(), 2, "only the two real keys are discovered");
    }

    #[tokio::test]
    async fn discovery_on_missing_dir_is_empty() {
        let discovery = OpenSshKeyDiscovery::new("/nonexistent/harbor/ssh");
        assert!(discovery.discover().await.unwrap().is_empty());
    }

    #[test]
    fn encrypted_key_is_flagged_without_passphrase() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("id_secure");
        std::fs::write(&path, ED25519_ENC_PRIV).unwrap();
        std::fs::write(dir.path().join("id_secure.pub"), ED25519_ENC_PUB).unwrap();

        let info = inspect_key_file(&path).unwrap();
        assert!(info.encrypted, "encrypted key must be flagged");
        // Public metadata is still available without the passphrase.
        assert_eq!(info.algorithm, "ssh-ed25519");
        assert_eq!(info.comment.as_deref(), Some("harbor-enc@example"));
        assert!(info.fingerprint.starts_with("SHA256:"));
    }
}
