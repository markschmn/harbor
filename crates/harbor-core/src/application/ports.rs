//! Ports: the trait boundaries the infrastructure layer must implement.
//!
//! Defining these as traits (rather than depending on concrete adapters) keeps
//! the dependency arrow pointing inward — the application orchestrates behaviour
//! without knowing whether profiles live in TOML, secrets in the macOS keychain,
//! or the SSH transport is russh. It also makes every service unit-testable with
//! lightweight fakes.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use secrecy::SecretString;
use tokio::sync::mpsc;

use crate::domain::auth::Credential;
use crate::domain::error::Result;
use crate::domain::file::DirEntry;
use crate::domain::host_key::{HostKey, HostKeyDecision, TofuResolution};
use crate::domain::key::DiscoveredKey;
use crate::domain::profile::{ProfileId, ServerProfile};
use crate::domain::session::{PtySize, SessionId};

/// Persistence for [`ServerProfile`]s.
#[async_trait]
pub trait ProfileRepository: Send + Sync {
    async fn list(&self) -> Result<Vec<ServerProfile>>;
    async fn get(&self, id: ProfileId) -> Result<ServerProfile>;
    async fn upsert(&self, profile: &ServerProfile) -> Result<()>;
    async fn delete(&self, id: ProfileId) -> Result<()>;
}

/// A typed reference to a secret stored in the OS keychain.
///
/// Encoding the *purpose* in the type avoids accidentally cross-wiring a
/// profile password with a key passphrase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretRef {
    /// The login password for a profile.
    ProfilePassword(ProfileId),
    /// The passphrase protecting a private key, keyed by its absolute path.
    KeyPassphrase(String),
    /// The app-lock PIN verifier (a salted hash, never the PIN itself).
    AppPin,
}

impl SecretRef {
    /// The account string used as the keychain entry key.
    pub fn account(&self) -> String {
        match self {
            SecretRef::ProfilePassword(id) => format!("profile-password:{id}"),
            SecretRef::KeyPassphrase(path) => format!("key-passphrase:{path}"),
            SecretRef::AppPin => "app-pin".to_string(),
        }
    }
}

/// Secure secret storage, backed by the OS keychain where available.
///
/// Implementations MUST NOT persist secrets in plaintext on disk.
#[async_trait]
pub trait SecretStore: Send + Sync {
    async fn set(&self, key: &SecretRef, secret: SecretString) -> Result<()>;
    async fn get(&self, key: &SecretRef) -> Result<Option<SecretString>>;
    async fn delete(&self, key: &SecretRef) -> Result<()>;
}

/// `known_hosts` management and the host-key verification decision.
#[async_trait]
pub trait KnownHostsStore: Send + Sync {
    /// Compare a presented host key against the trusted set and return a
    /// decision. This performs *no* persistence and never mutates state.
    async fn evaluate(&self, host: &str, port: u16, presented: &HostKey)
        -> Result<HostKeyDecision>;

    /// Persist a newly trusted key (Trust On First Use). Appends to the store
    /// following OpenSSH `known_hosts` conventions.
    async fn trust(&self, host: &str, port: u16, key: &HostKey) -> Result<()>;

    /// Remove every entry for a host (used by "forget host" in the UI).
    async fn forget(&self, host: &str, port: u16) -> Result<()>;
}

/// Asked, during a handshake, how to resolve an unknown host (TOFU). The
/// presentation layer implements this by prompting the user; headless callers
/// can supply a fixed policy.
#[async_trait]
pub trait HostKeyPrompter: Send + Sync {
    async fn resolve_unknown(&self, prompt: HostKeyPrompt) -> TofuResolution;
}

/// Data shown to the user when an unknown host key is encountered.
#[derive(Debug, Clone)]
pub struct HostKeyPrompt {
    pub host: String,
    pub port: u16,
    pub algorithm: String,
    pub fingerprint: String,
}

/// Discovery and inspection of on-disk SSH keys (typically `~/.ssh`).
#[async_trait]
pub trait KeyDiscovery: Send + Sync {
    /// Scan the standard locations and return discovered keys.
    async fn discover(&self) -> Result<Vec<DiscoveredKey>>;
    /// Inspect a single key file.
    async fn inspect(&self, path: &Path) -> Result<DiscoveredKey>;
}

/// Parameters for establishing an SSH connection.
#[derive(Debug, Clone)]
pub struct ConnectionParams {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub credential: Credential,
}

/// Establishes SSH connections. The transport is responsible for host-key
/// verification (delegating the decision to the injected [`KnownHostsStore`]
/// and [`HostKeyPrompter`]) before any authentication is attempted.
#[async_trait]
pub trait SshTransport: Send + Sync {
    async fn connect(
        &self,
        session_id: SessionId,
        params: ConnectionParams,
        known_hosts: Arc<dyn KnownHostsStore>,
        prompter: Arc<dyn HostKeyPrompter>,
    ) -> Result<Arc<dyn SshSession>>;
}

/// A live SSH connection capable of opening interactive shells and an SFTP
/// channel.
#[async_trait]
pub trait SshSession: Send + Sync {
    fn id(&self) -> SessionId;

    /// Open an interactive shell with a PTY of the given size.
    async fn open_shell(&self, size: PtySize) -> Result<ShellHandle>;

    /// Open (or reuse) the SFTP subsystem on this connection.
    async fn sftp(&self) -> Result<Arc<dyn SftpClient>>;

    /// Whether the underlying transport is still connected.
    fn is_connected(&self) -> bool;

    /// Gracefully disconnect.
    async fn close(&self) -> Result<()>;
}

/// Input sent toward a remote shell.
#[derive(Debug, Clone)]
pub enum ShellInput {
    /// Raw bytes typed by the user.
    Data(Vec<u8>),
    /// A terminal resize.
    Resize(PtySize),
    /// Signal end-of-input.
    Eof,
}

/// Output produced by a remote shell.
#[derive(Debug, Clone)]
pub enum ShellEvent {
    /// Bytes to render in the terminal (merged stdout + stderr).
    Output(Vec<u8>),
    /// The shell closed; carries the exit status if the server reported one.
    Closed { exit_code: Option<u32> },
}

/// A bidirectional handle to a remote interactive shell.
///
/// The write side ([`ShellInput`]) and the read side ([`ShellEvent`]) are
/// plain channels so the presentation layer can pump them into the GUI's event
/// system without holding a lock on the session.
#[derive(Debug)]
pub struct ShellHandle {
    pub session_id: SessionId,
    pub input: mpsc::Sender<ShellInput>,
    pub output: mpsc::Receiver<ShellEvent>,
}

/// Progress callback invoked repeatedly during a transfer with
/// `(transferred_bytes, total_bytes)`.
pub type ProgressFn = Arc<dyn Fn(u64, u64) + Send + Sync>;

/// A cancellation signal shared with a running transfer.
pub type CancelFlag = Arc<std::sync::atomic::AtomicBool>;

/// SFTP operations over an established session.
#[async_trait]
pub trait SftpClient: Send + Sync {
    /// List a remote directory.
    async fn read_dir(&self, path: &str) -> Result<Vec<DirEntry>>;

    /// Resolve the user's remote home / starting directory.
    async fn canonicalize(&self, path: &str) -> Result<String>;

    /// Create a directory (non-recursive).
    async fn mkdir(&self, path: &str) -> Result<()>;

    /// Remove a file.
    async fn remove_file(&self, path: &str) -> Result<()>;

    /// Remove an empty directory.
    async fn remove_dir(&self, path: &str) -> Result<()>;

    /// Rename / move.
    async fn rename(&self, from: &str, to: &str) -> Result<()>;

    /// `stat` a single entry.
    async fn stat(&self, path: &str) -> Result<DirEntry>;

    /// Download `remote` to `local`, reporting progress and honouring `cancel`.
    async fn download(
        &self,
        remote: &str,
        local: &Path,
        progress: ProgressFn,
        cancel: CancelFlag,
    ) -> Result<u64>;

    /// Upload `local` to `remote`, reporting progress and honouring `cancel`.
    async fn upload(
        &self,
        local: &Path,
        remote: &str,
        progress: ProgressFn,
        cancel: CancelFlag,
    ) -> Result<u64>;
}
