//! In-memory test doubles for the application ports.
//!
//! These are part of the public API on purpose: they let downstream code and
//! integration tests exercise the application services without a filesystem,
//! keychain or network. They are also used as a graceful fallback secret store
//! on platforms where no OS keychain is reachable (see
//! [`infrastructure::keychain`](crate::infrastructure::keychain)).

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use async_trait::async_trait;
use secrecy::{ExposeSecret, SecretString};

use crate::application::ports::{
    KeyDiscovery, KnownHostsStore, ProfileRepository, SecretRef, SecretStore,
};
use crate::domain::error::{HarborError, Result};
use crate::domain::host_key::{HostKey, HostKeyDecision, KnownHostEntry};
use crate::domain::key::DiscoveredKey;
use crate::domain::profile::{ProfileId, ServerProfile};
use crate::infrastructure::known_hosts::evaluate_entries;

/// An in-memory [`ProfileRepository`].
#[derive(Debug, Default)]
pub struct InMemoryProfileRepo {
    inner: Mutex<HashMap<ProfileId, ServerProfile>>,
}

#[async_trait]
impl ProfileRepository for InMemoryProfileRepo {
    async fn list(&self) -> Result<Vec<ServerProfile>> {
        Ok(self.inner.lock().unwrap().values().cloned().collect())
    }

    async fn get(&self, id: ProfileId) -> Result<ServerProfile> {
        self.inner
            .lock()
            .unwrap()
            .get(&id)
            .cloned()
            .ok_or_else(|| HarborError::NotFound(format!("profile {id}")))
    }

    async fn upsert(&self, profile: &ServerProfile) -> Result<()> {
        self.inner
            .lock()
            .unwrap()
            .insert(profile.id, profile.clone());
        Ok(())
    }

    async fn delete(&self, id: ProfileId) -> Result<()> {
        self.inner.lock().unwrap().remove(&id);
        Ok(())
    }
}

/// An in-memory [`SecretStore`]. Secrets are kept only in process memory.
#[derive(Debug, Default)]
pub struct InMemorySecretStore {
    inner: Mutex<HashMap<String, String>>,
}

#[async_trait]
impl SecretStore for InMemorySecretStore {
    async fn set(&self, key: &SecretRef, secret: SecretString) -> Result<()> {
        self.inner
            .lock()
            .unwrap()
            .insert(key.account(), secret.expose_secret().to_owned());
        Ok(())
    }

    async fn get(&self, key: &SecretRef) -> Result<Option<SecretString>> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .get(&key.account())
            .map(|s| SecretString::from(s.clone())))
    }

    async fn delete(&self, key: &SecretRef) -> Result<()> {
        self.inner.lock().unwrap().remove(&key.account());
        Ok(())
    }
}

/// An in-memory [`KnownHostsStore`] that runs the same evaluation logic as the
/// file-backed implementation, but against entries held in memory.
#[derive(Debug, Default)]
pub struct InMemoryKnownHosts {
    entries: Mutex<Vec<KnownHostEntry>>,
}

impl InMemoryKnownHosts {
    pub fn with_entries(entries: Vec<KnownHostEntry>) -> Self {
        Self {
            entries: Mutex::new(entries),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.lock().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[async_trait]
impl KnownHostsStore for InMemoryKnownHosts {
    async fn evaluate(
        &self,
        host: &str,
        port: u16,
        presented: &HostKey,
    ) -> Result<HostKeyDecision> {
        let entries = self.entries.lock().unwrap();
        Ok(evaluate_entries(&entries, host, port, presented))
    }

    async fn trust(&self, host: &str, port: u16, key: &HostKey) -> Result<()> {
        let host_field = crate::infrastructure::known_hosts::host_pattern(host, port);
        self.entries.lock().unwrap().push(KnownHostEntry {
            host_field,
            key: key.clone(),
            hashed: false,
            marker: None,
        });
        Ok(())
    }

    async fn forget(&self, host: &str, port: u16) -> Result<()> {
        let pattern = crate::infrastructure::known_hosts::host_pattern(host, port);
        self.entries
            .lock()
            .unwrap()
            .retain(|e| e.host_field != pattern);
        Ok(())
    }
}

/// A [`KeyDiscovery`] returning a fixed set of keys.
#[derive(Debug, Default)]
pub struct StaticKeyDiscovery {
    pub keys: Vec<DiscoveredKey>,
}

#[async_trait]
impl KeyDiscovery for StaticKeyDiscovery {
    async fn discover(&self) -> Result<Vec<DiscoveredKey>> {
        Ok(self.keys.clone())
    }

    async fn inspect(&self, path: &Path) -> Result<DiscoveredKey> {
        let p = path.to_string_lossy();
        self.keys
            .iter()
            .find(|k| k.private_key_path == p)
            .cloned()
            .ok_or_else(|| HarborError::NotFound(format!("key {p}")))
    }
}
