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
    use chacha20::ChaCha8Rng;
    use ssh_key::rand_core::SeedableRng;
    use ssh_key::{Algorithm, LineEnding};

    // A deterministic CSPRNG (the same crate ssh-key uses) — reproducible
    // throwaway keys generated at test time, so nothing is committed.
    fn rng() -> ChaCha8Rng {
        ChaCha8Rng::from_seed([7u8; 32])
    }

    /// Generate an unencrypted ed25519 keypair and write the private + `.pub`
    /// files into `dir`.
    fn write_keypair(dir: &Path, name: &str, comment: &str) -> PrivateKey {
        let mut key = PrivateKey::random(&mut rng(), Algorithm::Ed25519).unwrap();
        key.set_comment(comment);
        std::fs::write(
            dir.join(name),
            key.to_openssh(LineEnding::LF).unwrap().as_bytes(),
        )
        .unwrap();
        std::fs::write(
            dir.join(format!("{name}.pub")),
            key.public_key().to_openssh().unwrap(),
        )
        .unwrap();
        key
    }

    #[test]
    fn inspect_reports_algorithm_fingerprint_and_comment() {
        let dir = tempfile::tempdir().unwrap();
        let key = write_keypair(dir.path(), "id_ed25519", "me@laptop");
        let info = inspect_key_file(&dir.path().join("id_ed25519")).unwrap();

        assert_eq!(info.algorithm, "ssh-ed25519");
        assert!(!info.encrypted);
        assert_eq!(info.comment.as_deref(), Some("me@laptop"));
        assert!(info.public_key_path.is_some());
        // Matches what ssh-key computes for the same key.
        assert_eq!(
            info.fingerprint,
            key.public_key().fingerprint(HashAlg::Sha256).to_string()
        );
        assert!(info.fingerprint.starts_with("SHA256:"));
    }

    #[tokio::test]
    async fn discovery_finds_keys_and_skips_non_keys() {
        let dir = tempfile::tempdir().unwrap();
        write_keypair(dir.path(), "id_ed25519", "a@b");
        write_keypair(dir.path(), "work_key", "c@d");
        std::fs::write(dir.path().join("known_hosts"), "example.com ssh-ed25519 AAAA").unwrap();
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
        let key = PrivateKey::random(&mut rng(), Algorithm::Ed25519).unwrap();
        let encrypted = key.encrypt(&mut rng(), "passphrase").unwrap();
        let path = dir.path().join("id_secure");
        std::fs::write(
            &path,
            encrypted.to_openssh(LineEnding::LF).unwrap().as_bytes(),
        )
        .unwrap();

        let info = inspect_key_file(&path).unwrap();
        assert!(info.encrypted, "encrypted key must be flagged");
        // Public metadata is still available without the passphrase.
        assert_eq!(info.algorithm, "ssh-ed25519");
        assert!(info.fingerprint.starts_with("SHA256:"));
    }
}
