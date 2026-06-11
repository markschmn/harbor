//! The single error type used throughout the core crate.

use std::path::PathBuf;

/// Result alias used across the crate.
pub type Result<T> = std::result::Result<T, HarborError>;

/// All recoverable failures Harbor can surface.
///
/// Variants are deliberately coarse-grained and carry human readable context;
/// the presentation layer maps them to user-facing messages. Security-relevant
/// failures (host key mismatches, authentication failures) get their own
/// variants so callers can react differently.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum HarborError {
    /// A domain invariant was violated (e.g. empty profile name).
    #[error("validation error: {0}")]
    Validation(String),

    /// A requested entity could not be found.
    #[error("not found: {0}")]
    NotFound(String),

    /// Configuration / profile storage failed to load or persist.
    #[error("storage error: {0}")]
    Storage(String),

    /// (De)serialisation of TOML/JSON failed.
    #[error("serialization error: {0}")]
    Serialization(String),

    /// The OS keychain / secret store failed.
    #[error("secret store error: {0}")]
    SecretStore(String),

    /// The presented SSH host key did not match a previously trusted key.
    /// This is a hard security failure and must never be silently ignored.
    #[error("host key verification failed for {host}: {detail}")]
    HostKeyMismatch { host: String, detail: String },

    /// The host is not yet known and the caller must decide whether to trust
    /// it (Trust On First Use).
    #[error("host {0} is not in known_hosts")]
    HostKeyUnknown(String),

    /// SSH authentication was rejected by the server.
    #[error("authentication failed: {0}")]
    Authentication(String),

    /// A private key could not be parsed or decrypted.
    #[error("key error: {0}")]
    Key(String),

    /// The supplied passphrase for an encrypted key was missing or wrong.
    #[error("a passphrase is required to decrypt this key")]
    PassphraseRequired,

    /// Underlying SSH transport failure.
    #[error("ssh transport error: {0}")]
    Ssh(String),

    /// SFTP protocol failure.
    #[error("sftp error: {0}")]
    Sftp(String),

    /// A referenced session id is not connected.
    #[error("session {0} is not connected")]
    SessionNotConnected(String),

    /// Filesystem I/O failure with the offending path attached.
    #[error("io error at {path:?}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// A generic, already-formatted I/O error without a path.
    #[error("io error: {0}")]
    IoMsg(String),

    /// The operation was cancelled by the user.
    #[error("operation cancelled")]
    Cancelled,
}

impl HarborError {
    /// Helper for constructing a validation error from anything stringy.
    pub fn validation(msg: impl Into<String>) -> Self {
        HarborError::Validation(msg.into())
    }

    /// Stable machine-readable code, useful for the frontend to branch on
    /// without parsing human text.
    pub fn code(&self) -> &'static str {
        match self {
            HarborError::Validation(_) => "validation",
            HarborError::NotFound(_) => "not_found",
            HarborError::Storage(_) => "storage",
            HarborError::Serialization(_) => "serialization",
            HarborError::SecretStore(_) => "secret_store",
            HarborError::HostKeyMismatch { .. } => "host_key_mismatch",
            HarborError::HostKeyUnknown(_) => "host_key_unknown",
            HarborError::Authentication(_) => "authentication",
            HarborError::Key(_) => "key",
            HarborError::PassphraseRequired => "passphrase_required",
            HarborError::Ssh(_) => "ssh",
            HarborError::Sftp(_) => "sftp",
            HarborError::SessionNotConnected(_) => "session_not_connected",
            HarborError::Io { .. } | HarborError::IoMsg(_) => "io",
            HarborError::Cancelled => "cancelled",
        }
    }
}

impl From<std::io::Error> for HarborError {
    fn from(e: std::io::Error) -> Self {
        HarborError::IoMsg(e.to_string())
    }
}

impl From<toml::de::Error> for HarborError {
    fn from(e: toml::de::Error) -> Self {
        HarborError::Serialization(e.to_string())
    }
}

impl From<toml::ser::Error> for HarborError {
    fn from(e: toml::ser::Error) -> Self {
        HarborError::Serialization(e.to_string())
    }
}
