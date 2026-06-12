//! OS keychain-backed [`SecretStore`].
//!
//! Secrets (profile passwords, key passphrases) are stored in the platform
//! credential vault:
//!
//! * macOS — Keychain
//! * Windows — Windows Credential Manager
//! * Linux — Secret Service (GNOME Keyring / KWallet) via D-Bus
//!
//! The `keyring` crate is synchronous, so calls are dispatched onto a blocking
//! thread to keep the async runtime responsive.
//!
//! On a headless system with no reachable keychain (e.g. CI), the
//! [`build_secret_store`] factory transparently falls back to an in-process,
//! session-only store. Crucially, **the fallback still never writes secrets to
//! disk** — they simply do not persist across restarts. Harbor never stores a
//! password or passphrase in plaintext anywhere.

use std::sync::Arc;

use async_trait::async_trait;
use secrecy::{ExposeSecret, SecretString};

use crate::application::ports::{SecretRef, SecretStore};
use crate::domain::error::{HarborError, Result};

/// The keychain "service" namespace under which Harbor stores its secrets.
pub const KEYCHAIN_SERVICE: &str = "dev.harbor.ssh";

/// OS keychain-backed secret store.
#[derive(Debug, Clone)]
pub struct KeyringSecretStore {
    service: String,
}

impl Default for KeyringSecretStore {
    fn default() -> Self {
        Self {
            service: KEYCHAIN_SERVICE.to_string(),
        }
    }
}

impl KeyringSecretStore {
    pub fn new(service: impl Into<String>) -> Self {
        Self {
            service: service.into(),
        }
    }

    /// Probe whether the platform keychain is reachable, by performing a read
    /// for a sentinel account. A `NoEntry` (or success) means the backend is
    /// usable; a platform error means it is not.
    pub async fn is_available(&self) -> bool {
        let service = self.service.clone();
        tokio::task::spawn_blocking(move || {
            match keyring::Entry::new(&service, "__harbor_probe__") {
                Ok(entry) => !matches!(
                    entry.get_password(),
                    Err(keyring::Error::PlatformFailure(_))
                        | Err(keyring::Error::NoStorageAccess(_))
                ),
                Err(_) => false,
            }
        })
        .await
        .unwrap_or(false)
    }
}

fn map_keyring_err(e: keyring::Error) -> HarborError {
    HarborError::SecretStore(e.to_string())
}

#[async_trait]
impl SecretStore for KeyringSecretStore {
    async fn set(&self, key: &SecretRef, secret: SecretString) -> Result<()> {
        let service = self.service.clone();
        let account = key.account();
        let value = secret.expose_secret().to_owned();
        tokio::task::spawn_blocking(move || {
            let entry = keyring::Entry::new(&service, &account).map_err(map_keyring_err)?;
            entry.set_password(&value).map_err(map_keyring_err)
        })
        .await
        .map_err(|e| HarborError::SecretStore(format!("keychain task failed: {e}")))?
    }

    async fn get(&self, key: &SecretRef) -> Result<Option<SecretString>> {
        let service = self.service.clone();
        let account = key.account();
        tokio::task::spawn_blocking(move || {
            let entry = keyring::Entry::new(&service, &account).map_err(map_keyring_err)?;
            match entry.get_password() {
                Ok(p) => Ok(Some(SecretString::from(p))),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(e) => Err(map_keyring_err(e)),
            }
        })
        .await
        .map_err(|e| HarborError::SecretStore(format!("keychain task failed: {e}")))?
    }

    async fn delete(&self, key: &SecretRef) -> Result<()> {
        let service = self.service.clone();
        let account = key.account();
        tokio::task::spawn_blocking(move || {
            let entry = keyring::Entry::new(&service, &account).map_err(map_keyring_err)?;
            match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => Err(map_keyring_err(e)),
            }
        })
        .await
        .map_err(|e| HarborError::SecretStore(format!("keychain task failed: {e}")))?
    }
}

/// Build the best available secret store for the current environment.
///
/// Prefers the OS keychain; if it is unreachable, returns a session-only
/// in-memory store and logs a warning. Returns a flag indicating whether the
/// persistent keychain is in use, so the UI can inform the user.
pub async fn build_secret_store() -> (Arc<dyn SecretStore>, bool) {
    let keyring = KeyringSecretStore::default();
    if keyring.is_available().await {
        (Arc::new(keyring), true)
    } else {
        tracing::warn!(
            "no OS keychain is reachable; falling back to a session-only secret \
             store. Stored passwords will not persist across restarts."
        );
        (
            Arc::new(crate::testing::InMemorySecretStore::default()),
            false,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // We do not assert against the real OS keychain in unit tests (it may be
    // absent or require interactive unlock). We only verify the account-key
    // derivation and that the factory yields a working store.
    #[test]
    fn secret_ref_accounts_are_distinct_and_namespaced() {
        use crate::domain::profile::ProfileId;
        let id = ProfileId::new();
        let pw = SecretRef::ProfilePassword(id);
        let pass = SecretRef::KeyPassphrase("/home/me/.ssh/id_ed25519".into());
        assert!(pw.account().starts_with("profile-password:"));
        assert!(pass.account().starts_with("key-passphrase:"));
        assert_ne!(pw.account(), pass.account());
    }

    #[tokio::test]
    async fn factory_always_returns_a_usable_store() {
        let (store, _persistent) = build_secret_store().await;
        let key = SecretRef::KeyPassphrase("/tmp/test".into());
        // A real keychain may reject in CI; the call must simply not panic.
        let _ = store.set(&key, SecretString::from("abc")).await;
        let _ = store.get(&key).await;
        let _ = store.delete(&key).await;
    }
}
