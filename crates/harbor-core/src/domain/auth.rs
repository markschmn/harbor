//! Authentication value objects.
//!
//! A [`ServerProfile`](crate::domain::profile::ServerProfile) stores *how* to
//! authenticate ([`AuthMethod`]) but never the secret material itself. Secrets
//! (passwords, key passphrases) live in the OS keychain and are resolved at
//! connection time into a transient [`Credential`].

use std::path::PathBuf;

use secrecy::SecretString;
use serde::{Deserialize, Serialize};

/// The authentication strategy configured on a profile.
///
/// This is safe to serialise to disk: it contains references to key files and
/// flags, but never a password or passphrase.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthMethod {
    /// Authenticate against a running SSH agent (ssh-agent / Pageant).
    #[default]
    Agent,

    /// Interactive / stored password authentication. The password itself is
    /// kept in the keychain keyed by the profile id, never here.
    Password,

    /// Public-key authentication using a key on disk.
    PublicKey {
        /// Path to the private key (OpenSSH format).
        key_path: PathBuf,
        /// Whether the key is known to be passphrase-encrypted. Used purely as
        /// a UI hint; the real check happens when the key is parsed.
        #[serde(default)]
        encrypted: bool,
    },
}

impl AuthMethod {
    /// A short, stable identifier for the auth type (used by the UI).
    pub fn kind(&self) -> &'static str {
        match self {
            AuthMethod::Agent => "agent",
            AuthMethod::Password => "password",
            AuthMethod::PublicKey { .. } => "public_key",
        }
    }

    /// Whether connecting with this method may require prompting the user for a
    /// secret (password or key passphrase) that is not on disk.
    pub fn may_need_secret(&self) -> bool {
        matches!(
            self,
            AuthMethod::Password
                | AuthMethod::PublicKey {
                    encrypted: true,
                    ..
                }
        )
    }
}

/// A resolved, in-memory credential used for a single connection attempt.
///
/// Secret fields are wrapped in [`SecretString`] so they are zeroed on drop and
/// never accidentally logged or serialised.
#[derive(Clone)]
pub enum Credential {
    /// Use the SSH agent; no secret material needed in-process.
    Agent,
    /// A password.
    Password(SecretString),
    /// A private key path plus an optional passphrase to decrypt it.
    PrivateKey {
        key_path: PathBuf,
        passphrase: Option<SecretString>,
    },
}

impl std::fmt::Debug for Credential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never print secret material.
        match self {
            Credential::Agent => f.write_str("Credential::Agent"),
            Credential::Password(_) => f.write_str("Credential::Password(***)"),
            Credential::PrivateKey { key_path, .. } => f
                .debug_struct("Credential::PrivateKey")
                .field("key_path", key_path)
                .field("passphrase", &"***")
                .finish(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_method_round_trips_through_toml() {
        let method = AuthMethod::PublicKey {
            key_path: PathBuf::from("/home/me/.ssh/id_ed25519"),
            encrypted: true,
        };
        let serialised = toml::to_string(&method).unwrap();
        let parsed: AuthMethod = toml::from_str(&serialised).unwrap();
        assert_eq!(method, parsed);
    }

    #[test]
    fn debug_credential_does_not_leak_secret() {
        let cred = Credential::Password(SecretString::from("hunter2"));
        let rendered = format!("{cred:?}");
        assert!(!rendered.contains("hunter2"));
        assert!(rendered.contains("***"));
    }

    #[test]
    fn may_need_secret_is_accurate() {
        assert!(AuthMethod::Password.may_need_secret());
        assert!(!AuthMethod::Agent.may_need_secret());
        assert!(AuthMethod::PublicKey {
            key_path: "/k".into(),
            encrypted: true
        }
        .may_need_secret());
        assert!(!AuthMethod::PublicKey {
            key_path: "/k".into(),
            encrypted: false
        }
        .may_need_secret());
    }
}
