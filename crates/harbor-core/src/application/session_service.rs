//! Session use cases: establishing connections, tracking their lifecycle and
//! handing out shell + SFTP handles.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::domain::error::{HarborError, Result};
use crate::domain::profile::ProfileId;
use crate::domain::session::{PtySize, SessionId, SessionInfo, SessionStatus};

use super::ports::{
    ConnectionParams, HostKeyPrompter, KnownHostsStore, ShellHandle, SftpClient, SshSession,
    SshTransport,
};

/// An event describing a change to a session's lifecycle. The presentation
/// layer forwards these to the UI.
#[derive(Debug, Clone)]
pub enum SessionEvent {
    StatusChanged { id: SessionId, status: SessionStatus },
}

/// Internal registry entry pairing a live transport with its metadata.
struct Entry {
    session: Arc<dyn SshSession>,
    info: SessionInfo,
}

/// Manages the set of live SSH sessions (one per terminal tab).
pub struct SessionService {
    transport: Arc<dyn SshTransport>,
    known_hosts: Arc<dyn KnownHostsStore>,
    prompter: Arc<dyn HostKeyPrompter>,
    sessions: Mutex<HashMap<SessionId, Entry>>,
}

impl std::fmt::Debug for SessionService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SessionService")
    }
}

impl SessionService {
    pub fn new(
        transport: Arc<dyn SshTransport>,
        known_hosts: Arc<dyn KnownHostsStore>,
        prompter: Arc<dyn HostKeyPrompter>,
    ) -> Self {
        Self {
            transport,
            known_hosts,
            prompter,
            sessions: Mutex::new(HashMap::new()),
        }
    }

    /// Open a new SSH connection. Host-key verification happens inside the
    /// transport before authentication; a mismatch surfaces here as an error
    /// and the session is never registered.
    pub async fn connect(
        &self,
        params: ConnectionParams,
        profile_id: Option<ProfileId>,
        title: impl Into<String>,
    ) -> Result<SessionInfo> {
        let id = SessionId::new();
        let session = self
            .transport
            .connect(
                id,
                params,
                Arc::clone(&self.known_hosts),
                Arc::clone(&self.prompter),
            )
            .await?;

        let info = SessionInfo {
            id,
            profile_id,
            title: title.into(),
            status: SessionStatus::Connected,
        };
        self.sessions.lock().await.insert(
            id,
            Entry {
                session,
                info: info.clone(),
            },
        );
        Ok(info)
    }

    /// Look up a live session, erroring if it is gone or disconnected.
    async fn session(&self, id: SessionId) -> Result<Arc<dyn SshSession>> {
        let guard = self.sessions.lock().await;
        let entry = guard
            .get(&id)
            .ok_or_else(|| HarborError::SessionNotConnected(id.to_string()))?;
        if !entry.session.is_connected() {
            return Err(HarborError::SessionNotConnected(id.to_string()));
        }
        Ok(Arc::clone(&entry.session))
    }

    /// Open an interactive shell on a session.
    pub async fn open_shell(&self, id: SessionId, size: PtySize) -> Result<ShellHandle> {
        self.session(id).await?.open_shell(size).await
    }

    /// Get the SFTP client for a session.
    pub async fn sftp(&self, id: SessionId) -> Result<Arc<dyn SftpClient>> {
        self.session(id).await?.sftp().await
    }

    /// Disconnect and forget a session. Idempotent.
    pub async fn disconnect(&self, id: SessionId) -> Result<()> {
        let entry = self.sessions.lock().await.remove(&id);
        if let Some(entry) = entry {
            entry.session.close().await?;
        }
        Ok(())
    }

    /// Snapshot of all known sessions, refreshing the connected flag.
    pub async fn list(&self) -> Vec<SessionInfo> {
        let mut guard = self.sessions.lock().await;
        for entry in guard.values_mut() {
            if !entry.session.is_connected()
                && matches!(entry.info.status, SessionStatus::Connected)
            {
                entry.info.status = SessionStatus::Disconnected {
                    reason: "transport closed".into(),
                };
            }
        }
        guard.values().map(|e| e.info.clone()).collect()
    }

    /// Whether a session id currently maps to a live connection.
    pub async fn is_connected(&self, id: SessionId) -> bool {
        self.session(id).await.is_ok()
    }

    /// Number of registered sessions (connected or not).
    pub async fn count(&self) -> usize {
        self.sessions.lock().await.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::InMemoryKnownHosts;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// A fake transport whose sessions report connectivity from a flag.
    struct FakeTransport;

    struct FakeSession {
        id: SessionId,
        connected: AtomicBool,
    }

    #[async_trait]
    impl SshTransport for FakeTransport {
        async fn connect(
            &self,
            session_id: SessionId,
            _params: ConnectionParams,
            _known_hosts: Arc<dyn KnownHostsStore>,
            _prompter: Arc<dyn HostKeyPrompter>,
        ) -> Result<Arc<dyn SshSession>> {
            Ok(Arc::new(FakeSession {
                id: session_id,
                connected: AtomicBool::new(true),
            }))
        }
    }

    #[async_trait]
    impl SshSession for FakeSession {
        fn id(&self) -> SessionId {
            self.id
        }
        async fn open_shell(&self, _size: PtySize) -> Result<ShellHandle> {
            let (_in_tx, _in_rx) = tokio::sync::mpsc::channel(1);
            let (_out_tx, out_rx) = tokio::sync::mpsc::channel(1);
            Ok(ShellHandle {
                session_id: self.id,
                input: _in_tx,
                output: out_rx,
            })
        }
        async fn sftp(&self) -> Result<Arc<dyn SftpClient>> {
            Err(HarborError::Sftp("not supported in fake".into()))
        }
        fn is_connected(&self) -> bool {
            self.connected.load(Ordering::SeqCst)
        }
        async fn close(&self) -> Result<()> {
            self.connected.store(false, Ordering::SeqCst);
            Ok(())
        }
    }

    fn service() -> SessionService {
        SessionService::new(
            Arc::new(FakeTransport),
            Arc::new(InMemoryKnownHosts::default()),
            Arc::new(crate::infrastructure::host_key_policy::AcceptNewPolicy),
        )
    }

    fn params() -> ConnectionParams {
        ConnectionParams {
            host: "h".into(),
            port: 22,
            username: "u".into(),
            credential: crate::domain::auth::Credential::Agent,
        }
    }

    #[tokio::test]
    async fn connect_register_and_disconnect() {
        let svc = service();
        let info = svc.connect(params(), None, "tab").await.unwrap();
        assert_eq!(svc.count().await, 1);
        assert!(svc.is_connected(info.id).await);

        svc.open_shell(info.id, PtySize::default()).await.unwrap();

        svc.disconnect(info.id).await.unwrap();
        assert_eq!(svc.count().await, 0);
        assert!(!svc.is_connected(info.id).await);
    }

    #[tokio::test]
    async fn operations_on_unknown_session_error() {
        let svc = service();
        let bogus = SessionId::new();
        assert!(svc.open_shell(bogus, PtySize::default()).await.is_err());
        assert!(svc.sftp(bogus).await.is_err());
        // disconnect is idempotent / non-erroring
        svc.disconnect(bogus).await.unwrap();
    }
}
