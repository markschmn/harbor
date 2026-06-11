//! Key-management use cases: discovery, inspection and fingerprinting of
//! on-disk SSH keys.

use std::path::Path;
use std::sync::Arc;

use crate::domain::error::Result;
use crate::domain::key::DiscoveredKey;

use super::ports::KeyDiscovery;

/// Thin orchestration over a [`KeyDiscovery`] adapter.
pub struct KeyService {
    discovery: Arc<dyn KeyDiscovery>,
}

impl std::fmt::Debug for KeyService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("KeyService")
    }
}

impl KeyService {
    pub fn new(discovery: Arc<dyn KeyDiscovery>) -> Self {
        Self { discovery }
    }

    /// Discover keys in the standard locations, sorted by path for stable UI
    /// ordering.
    pub async fn list(&self) -> Result<Vec<DiscoveredKey>> {
        let mut keys = self.discovery.discover().await?;
        keys.sort_by(|a, b| a.private_key_path.cmp(&b.private_key_path));
        Ok(keys)
    }

    /// Inspect a single key file the user pointed at.
    pub async fn inspect(&self, path: impl AsRef<Path>) -> Result<DiscoveredKey> {
        self.discovery.inspect(path.as_ref()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::StaticKeyDiscovery;

    fn key(path: &str) -> DiscoveredKey {
        DiscoveredKey {
            private_key_path: path.into(),
            public_key_path: None,
            algorithm: "ssh-ed25519".into(),
            fingerprint: "SHA256:zzz".into(),
            comment: None,
            bits: None,
            encrypted: false,
        }
    }

    #[tokio::test]
    async fn list_is_sorted_by_path() {
        let svc = KeyService::new(Arc::new(StaticKeyDiscovery {
            keys: vec![key("/z/id"), key("/a/id")],
        }));
        let listed = svc.list().await.unwrap();
        assert_eq!(listed[0].private_key_path, "/a/id");
    }

    #[tokio::test]
    async fn inspect_finds_by_path() {
        let svc = KeyService::new(Arc::new(StaticKeyDiscovery {
            keys: vec![key("/a/id")],
        }));
        assert!(svc.inspect("/a/id").await.is_ok());
        assert!(svc.inspect("/missing").await.is_err());
    }
}
