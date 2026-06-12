//! The russh-backed SSH transport.
//!
//! ## Why russh?
//!
//! Harbor uses [russh](https://crates.io/crates/russh), a pure-Rust, async
//! (tokio) SSH implementation, rather than `ssh2`/libssh2 (C, largely
//! synchronous) or wrapping the system `ssh` binary (`openssh`):
//!
//! * **Cross-platform, Windows-first** — no C build dependency and no reliance
//!   on an OpenSSH binary being installed, which matters most on Windows.
//! * **Async** — integrates naturally with the tokio runtime that drives the
//!   rest of the app, giving non-blocking terminals and transfers.
//! * **Memory-safe** — no `unsafe` FFI surface for the security-critical path.
//! * **Controllable host-key verification** — the [`client::Handler`] trait
//!   gives us a precise hook ([`ClientHandler::check_server_key`]) to enforce
//!   our `known_hosts` policy *before* authentication.
//!
//! The trade-off is that russh implements a deliberately modern subset of SSH
//! algorithms; this is acceptable (and arguably desirable) for a new client.

use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::Duration;

use async_trait::async_trait;
use russh::client::{self, AuthResult, Handle};
use russh::keys::agent::client::AgentClient;
use russh::keys::PrivateKeyWithHashAlg;
use russh::{ChannelMsg, Disconnect};
use secrecy::ExposeSecret;
use tokio::sync::mpsc;
use tokio::sync::Mutex as AsyncMutex;

use crate::application::ports::{
    ConnectionParams, HostKeyPrompt, HostKeyPrompter, KnownHostsStore, SftpClient, ShellEvent,
    ShellHandle, ShellInput, SshSession, SshTransport,
};
use crate::domain::auth::Credential;
use crate::domain::error::{HarborError, Result};
use crate::domain::host_key::{HostKey, HostKeyDecision, TofuResolution};
use crate::domain::session::{PtySize, SessionId};

use super::sftp::RusshSftpClient;

fn ssh_err(e: russh::Error) -> HarborError {
    HarborError::Ssh(e.to_string())
}

/// The russh implementation of [`SshTransport`].
#[derive(Debug, Default, Clone)]
pub struct RusshTransport;

#[async_trait]
impl SshTransport for RusshTransport {
    async fn connect(
        &self,
        session_id: SessionId,
        params: ConnectionParams,
        known_hosts: Arc<dyn KnownHostsStore>,
        prompter: Arc<dyn HostKeyPrompter>,
    ) -> Result<Arc<dyn SshSession>> {
        let outcome = Arc::new(StdMutex::new(None));
        let handler = ClientHandler {
            host: params.host.clone(),
            port: params.port,
            known_hosts,
            prompter,
            outcome: Arc::clone(&outcome),
        };

        let config = Arc::new(client::Config {
            inactivity_timeout: Some(Duration::from_secs(3600)),
            keepalive_interval: Some(Duration::from_secs(30)),
            ..Default::default()
        });

        // `connect` performs the transport handshake, which invokes
        // `check_server_key`. A rejected host key surfaces as an error here; we
        // translate the recorded decision into a precise, typed error.
        let mut handle =
            match client::connect(config, (params.host.as_str(), params.port), handler).await {
                Ok(h) => h,
                Err(e) => {
                    if let Some(decision) = outcome.lock().unwrap().take() {
                        return Err(decision_to_error(&params.host, decision));
                    }
                    return Err(HarborError::Ssh(format!(
                        "could not connect to {}:{}: {e}",
                        params.host, params.port
                    )));
                }
            };

        authenticate(&mut handle, &params).await?;

        Ok(Arc::new(RusshSession {
            id: session_id,
            handle,
            sftp: AsyncMutex::new(None),
        }))
    }
}

/// Translate a non-trusting host-key decision into a typed error.
fn decision_to_error(host: &str, decision: HostKeyDecision) -> HarborError {
    match decision {
        HostKeyDecision::Mismatch { fingerprint, .. } => HarborError::HostKeyMismatch {
            host: host.to_string(),
            detail: format!(
                "the server presented a key ({fingerprint}) that does not match the one on \
                 record. This may indicate a man-in-the-middle attack. Connection refused."
            ),
        },
        HostKeyDecision::Revoked { fingerprint } => HarborError::HostKeyMismatch {
            host: host.to_string(),
            detail: format!("the server's host key ({fingerprint}) has been revoked"),
        },
        HostKeyDecision::Unknown { .. } => HarborError::HostKeyUnknown(host.to_string()),
        HostKeyDecision::Trusted => {
            HarborError::Ssh("internal error: trusted key reached error path".into())
        }
    }
}

/// The russh client handler. Its sole responsibility is host-key verification;
/// all authentication is driven explicitly from [`authenticate`].
struct ClientHandler {
    host: String,
    port: u16,
    known_hosts: Arc<dyn KnownHostsStore>,
    prompter: Arc<dyn HostKeyPrompter>,
    /// Records *why* a key was rejected so `connect` can produce a typed error.
    outcome: Arc<StdMutex<Option<HostKeyDecision>>>,
}

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &ssh_key::PublicKey,
    ) -> std::result::Result<bool, Self::Error> {
        let presented = public_key_to_host_key(server_public_key);

        let decision = match self
            .known_hosts
            .evaluate(&self.host, self.port, &presented)
            .await
        {
            Ok(d) => d,
            Err(e) => {
                tracing::error!("known_hosts evaluation failed: {e}");
                return Ok(false);
            }
        };

        match decision {
            HostKeyDecision::Trusted => Ok(true),
            HostKeyDecision::Mismatch { .. } | HostKeyDecision::Revoked { .. } => {
                tracing::warn!(
                    host = %self.host,
                    "refusing connection: host key verification failed"
                );
                *self.outcome.lock().unwrap() = Some(decision);
                Ok(false)
            }
            HostKeyDecision::Unknown { ref fingerprint } => {
                let prompt = HostKeyPrompt {
                    host: self.host.clone(),
                    port: self.port,
                    algorithm: presented.algorithm.clone(),
                    fingerprint: fingerprint.clone(),
                };
                match self.prompter.resolve_unknown(prompt).await {
                    TofuResolution::TrustAndSave => {
                        if let Err(e) = self
                            .known_hosts
                            .trust(&self.host, self.port, &presented)
                            .await
                        {
                            tracing::warn!("failed to persist trusted host key: {e}");
                        }
                        Ok(true)
                    }
                    TofuResolution::TrustOnce => Ok(true),
                    TofuResolution::Reject => {
                        *self.outcome.lock().unwrap() = Some(decision);
                        Ok(false)
                    }
                }
            }
        }
    }
}

/// Convert russh's `ssh_key::PublicKey` into our domain [`HostKey`].
fn public_key_to_host_key(key: &ssh_key::PublicKey) -> HostKey {
    // `to_openssh` yields "<algorithm> <base64> [comment]".
    match key.to_openssh() {
        Ok(line) => {
            let mut parts = line.split_whitespace();
            let algorithm = parts.next().unwrap_or_default().to_string();
            let key_base64 = parts.next().unwrap_or_default().to_string();
            HostKey::new(algorithm, key_base64)
        }
        Err(_) => HostKey::new(key.algorithm().as_str().to_string(), String::new()),
    }
}

/// Drive authentication according to the configured credential.
async fn authenticate(handle: &mut Handle<ClientHandler>, params: &ConnectionParams) -> Result<()> {
    match &params.credential {
        Credential::Password(secret) => {
            let res = handle
                .authenticate_password(&params.username, secret.expose_secret())
                .await
                .map_err(ssh_err)?;
            ensure_authenticated(res)
        }
        Credential::PrivateKey {
            key_path,
            passphrase,
        } => {
            let passphrase = passphrase.as_ref().map(|p| p.expose_secret().to_string());
            let key = load_private_key(key_path, passphrase.as_deref())?;
            let is_rsa = key.algorithm().is_rsa();
            let with = PrivateKeyWithHashAlg::new(
                Arc::new(key),
                is_rsa.then_some(ssh_key::HashAlg::Sha256),
            );
            let res = handle
                .authenticate_publickey(&params.username, with)
                .await
                .map_err(ssh_err)?;
            ensure_authenticated(res)
        }
        Credential::Agent => authenticate_with_agent(handle, &params.username).await,
    }
}

fn ensure_authenticated(res: AuthResult) -> Result<()> {
    if res.success() {
        Ok(())
    } else {
        Err(HarborError::Authentication(
            "the server rejected the supplied credentials".into(),
        ))
    }
}

/// Load and (if needed) decrypt a private key from disk, mapping the
/// "encrypted but no passphrase" case to a dedicated error so the UI can prompt.
fn load_private_key(path: &Path, passphrase: Option<&str>) -> Result<russh::keys::PrivateKey> {
    match russh::keys::load_secret_key(path, passphrase) {
        Ok(key) => Ok(key),
        Err(e) => {
            let lower = e.to_string().to_lowercase();
            if passphrase.is_none() && (lower.contains("encrypt") || lower.contains("passphrase")) {
                Err(HarborError::PassphraseRequired)
            } else if lower.contains("crypt")
                || lower.contains("mac")
                || lower.contains("password")
                || lower.contains("tag")
            {
                Err(HarborError::Key(
                    "could not decrypt the private key — the passphrase may be incorrect".into(),
                ))
            } else {
                Err(HarborError::Key(format!(
                    "could not load private key {}: {e}",
                    path.display()
                )))
            }
        }
    }
}

/// Authenticate by delegating signing to a running SSH agent.
///
/// The agent connection differs by platform — a Unix socket (`SSH_AUTH_SOCK`)
/// on Unix, the Windows OpenSSH named pipe or Pageant on Windows — but the
/// identity-iteration logic is identical, so it lives in the generic
/// [`agent_authenticate`] helper.
#[cfg(unix)]
async fn authenticate_with_agent(handle: &mut Handle<ClientHandler>, username: &str) -> Result<()> {
    let mut agent = AgentClient::connect_env().await.map_err(|e| {
        HarborError::Authentication(format!("could not connect to an SSH agent: {e}"))
    })?;
    agent_authenticate(handle, username, &mut agent).await
}

#[cfg(windows)]
async fn authenticate_with_agent(handle: &mut Handle<ClientHandler>, username: &str) -> Result<()> {
    // Prefer the Windows OpenSSH agent (named pipe); fall back to Pageant.
    if let Ok(mut agent) = AgentClient::connect_named_pipe(r"\\.\pipe\openssh-ssh-agent").await {
        return agent_authenticate(handle, username, &mut agent).await;
    }
    let mut agent = AgentClient::connect_pageant().await.map_err(|e| {
        HarborError::Authentication(format!(
            "could not connect to an SSH agent (OpenSSH named pipe or Pageant): {e}"
        ))
    })?;
    agent_authenticate(handle, username, &mut agent).await
}

/// Iterate the agent's identities and try each against the server.
///
/// Generic over the agent's stream type so it serves every platform's concrete
/// agent client. The bound matches russh's `Signer` impl for `AgentClient`.
async fn agent_authenticate<R>(
    handle: &mut Handle<ClientHandler>,
    username: &str,
    agent: &mut AgentClient<R>,
) -> Result<()>
where
    R: russh::keys::agent::client::AgentStream + Unpin + Send,
{
    let identities = agent
        .request_identities()
        .await
        .map_err(|e| HarborError::Authentication(format!("SSH agent error: {e}")))?;

    if identities.is_empty() {
        return Err(HarborError::Authentication(
            "the SSH agent has no identities loaded (try `ssh-add`)".into(),
        ));
    }

    for identity in identities {
        let public = identity.public_key().into_owned();
        let hash_alg = public
            .algorithm()
            .is_rsa()
            .then_some(ssh_key::HashAlg::Sha256);
        if let Ok(res) = handle
            .authenticate_publickey_with(username, public, hash_alg, agent)
            .await
        {
            if res.success() {
                return Ok(());
            }
        }
    }

    Err(HarborError::Authentication(
        "no identity offered by the SSH agent was accepted by the server".into(),
    ))
}

/// A live russh session.
struct RusshSession {
    id: SessionId,
    handle: Handle<ClientHandler>,
    /// Lazily-opened, shared SFTP subsystem.
    sftp: AsyncMutex<Option<Arc<dyn SftpClient>>>,
}

impl std::fmt::Debug for RusshSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RusshSession")
            .field("id", &self.id)
            .finish()
    }
}

#[async_trait]
impl SshSession for RusshSession {
    fn id(&self) -> SessionId {
        self.id
    }

    async fn open_shell(&self, size: PtySize) -> Result<ShellHandle> {
        let channel = self.handle.channel_open_session().await.map_err(ssh_err)?;
        channel
            .request_pty(
                false,
                "xterm-256color",
                size.cols,
                size.rows,
                size.pixel_width,
                size.pixel_height,
                &[],
            )
            .await
            .map_err(ssh_err)?;
        channel.request_shell(false).await.map_err(ssh_err)?;

        let (input_tx, input_rx) = mpsc::channel::<ShellInput>(512);
        let (output_tx, output_rx) = mpsc::channel::<ShellEvent>(512);
        tokio::spawn(pump_shell(channel, input_rx, output_tx));

        Ok(ShellHandle {
            session_id: self.id,
            input: input_tx,
            output: output_rx,
        })
    }

    async fn sftp(&self) -> Result<Arc<dyn SftpClient>> {
        let mut guard = self.sftp.lock().await;
        if let Some(existing) = guard.as_ref() {
            return Ok(Arc::clone(existing));
        }
        let channel = self.handle.channel_open_session().await.map_err(ssh_err)?;
        channel
            .request_subsystem(true, "sftp")
            .await
            .map_err(ssh_err)?;
        let stream = channel.into_stream();
        let session = russh_sftp::client::SftpSession::new(stream)
            .await
            .map_err(|e| HarborError::Sftp(format!("could not start SFTP subsystem: {e}")))?;
        let client: Arc<dyn SftpClient> = Arc::new(RusshSftpClient::new(session));
        *guard = Some(Arc::clone(&client));
        Ok(client)
    }

    fn is_connected(&self) -> bool {
        !self.handle.is_closed()
    }

    async fn close(&self) -> Result<()> {
        let _ = self
            .handle
            .disconnect(Disconnect::ByApplication, "", "")
            .await;
        Ok(())
    }
}

/// The per-shell I/O pump. Owns the channel for the lifetime of the shell and
/// bridges it to the [`ShellInput`]/[`ShellEvent`] channels.
///
/// `channel.wait()` is an mpsc receive under the hood and therefore
/// cancellation-safe inside `select!`.
async fn pump_shell(
    mut channel: russh::Channel<client::Msg>,
    mut input: mpsc::Receiver<ShellInput>,
    output: mpsc::Sender<ShellEvent>,
) {
    let mut exit_code: Option<u32> = None;
    loop {
        tokio::select! {
            msg = channel.wait() => {
                match msg {
                    Some(ChannelMsg::Data { data }) => {
                        if output.send(ShellEvent::Output(data.to_vec())).await.is_err() {
                            break;
                        }
                    }
                    Some(ChannelMsg::ExtendedData { data, .. }) => {
                        if output.send(ShellEvent::Output(data.to_vec())).await.is_err() {
                            break;
                        }
                    }
                    Some(ChannelMsg::ExitStatus { exit_status }) => {
                        exit_code = Some(exit_status);
                    }
                    Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => break,
                    Some(_) => {}
                }
            }
            cmd = input.recv() => {
                match cmd {
                    Some(ShellInput::Data(bytes)) => {
                        if channel.data(&bytes[..]).await.is_err() {
                            break;
                        }
                    }
                    Some(ShellInput::Resize(size)) => {
                        let _ = channel
                            .window_change(size.cols, size.rows, size.pixel_width, size.pixel_height)
                            .await;
                    }
                    Some(ShellInput::Eof) => {
                        let _ = channel.eof().await;
                    }
                    None => {
                        // Input side dropped: signal EOF and stop.
                        let _ = channel.eof().await;
                        break;
                    }
                }
            }
        }
    }
    let _ = output.send(ShellEvent::Closed { exit_code }).await;
}
