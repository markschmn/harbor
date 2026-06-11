//! Application state shared across all Tauri commands, plus the interactive
//! host-key prompter that bridges russh's handshake to the UI.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tauri::{AppHandle, Emitter};
use tokio::sync::{oneshot, Mutex};
use uuid::Uuid;

use harbor_core::application::ports::{
    HostKeyPrompt, HostKeyPrompter, KnownHostsStore, SecretStore, ShellInput,
};
use harbor_core::domain::host_key::TofuResolution;
use harbor_core::domain::session::SessionId;
use harbor_core::infrastructure::keychain::build_secret_store;
use harbor_core::infrastructure::keys::OpenSshKeyDiscovery;
use harbor_core::infrastructure::known_hosts::FileKnownHostsStore;
use harbor_core::infrastructure::ssh::RusshTransport;
use harbor_core::infrastructure::storage::TomlProfileRepository;
use harbor_core::prelude::*;
use harbor_core::Result;

/// Event name emitted when an unknown host key needs a user decision.
pub const EVENT_HOST_KEY_PROMPT: &str = "harbor://host-key-prompt";

/// How long to wait for the user to answer a host-key prompt before defaulting
/// to refusing the connection.
const HOST_KEY_PROMPT_TIMEOUT: Duration = Duration::from_secs(180);

/// The interactive host-key prompter.
///
/// When russh encounters an unknown host during the handshake, it calls
/// [`HostKeyPrompter::resolve_unknown`], which emits an event to the frontend
/// and parks on a oneshot channel. The `respond_host_key` command fulfils that
/// channel with the user's decision.
pub struct EventPrompter {
    app: Mutex<Option<AppHandle>>,
    pending: Mutex<HashMap<String, oneshot::Sender<TofuResolution>>>,
}

/// Payload sent to the UI for a host-key decision.
#[derive(Debug, Clone, serde::Serialize)]
pub struct HostKeyPromptPayload {
    pub request_id: String,
    pub host: String,
    pub port: u16,
    pub algorithm: String,
    pub fingerprint: String,
}

impl EventPrompter {
    pub fn new() -> Self {
        Self {
            app: Mutex::new(None),
            pending: Mutex::new(HashMap::new()),
        }
    }

    /// Inject the app handle once the Tauri app has been built.
    pub async fn set_app_handle(&self, app: AppHandle) {
        *self.app.lock().await = Some(app);
    }

    /// Resolve a pending prompt (called from the `respond_host_key` command).
    pub async fn respond(&self, request_id: &str, resolution: TofuResolution) -> bool {
        if let Some(tx) = self.pending.lock().await.remove(request_id) {
            tx.send(resolution).is_ok()
        } else {
            false
        }
    }
}

impl std::fmt::Debug for EventPrompter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EventPrompter")
    }
}

#[async_trait]
impl HostKeyPrompter for EventPrompter {
    async fn resolve_unknown(&self, prompt: HostKeyPrompt) -> TofuResolution {
        let request_id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        self.pending
            .lock()
            .await
            .insert(request_id.clone(), tx);

        let payload = HostKeyPromptPayload {
            request_id: request_id.clone(),
            host: prompt.host,
            port: prompt.port,
            algorithm: prompt.algorithm,
            fingerprint: prompt.fingerprint,
        };

        // Emit to the UI. If we cannot (no app handle / emit failed), fail safe
        // by refusing the connection.
        let emitted = {
            let guard = self.app.lock().await;
            match guard.as_ref() {
                Some(app) => app.emit(EVENT_HOST_KEY_PROMPT, payload).is_ok(),
                None => false,
            }
        };
        if !emitted {
            self.pending.lock().await.remove(&request_id);
            return TofuResolution::Reject;
        }

        match tokio::time::timeout(HOST_KEY_PROMPT_TIMEOUT, rx).await {
            Ok(Ok(resolution)) => resolution,
            // Timed out or the sender was dropped: refuse.
            _ => {
                self.pending.lock().await.remove(&request_id);
                TofuResolution::Reject
            }
        }
    }
}

/// The complete, shared application state.
pub struct AppState {
    pub profiles: Arc<ProfileService>,
    pub sessions: Arc<SessionService>,
    pub transfers: Arc<TransferService>,
    pub keys: Arc<KeyService>,
    pub known_hosts: Arc<dyn KnownHostsStore>,
    pub secrets: Arc<dyn SecretStore>,
    pub prompter: Arc<EventPrompter>,
    /// Per-session shell input channels, used by `send_input` / `resize`.
    pub shells: Mutex<HashMap<SessionId, tokio::sync::mpsc::Sender<ShellInput>>>,
    /// Whether secrets persist in the OS keychain (false → session-only memory).
    pub keychain_persistent: bool,
}

impl AppState {
    pub async fn new() -> Result<Self> {
        let repo = Arc::new(TomlProfileRepository::with_default_location()?);
        let (secrets, keychain_persistent) = build_secret_store().await;
        let known_hosts: Arc<dyn KnownHostsStore> =
            Arc::new(FileKnownHostsStore::with_default_location()?);
        let key_discovery = Arc::new(OpenSshKeyDiscovery::with_default_location()?);
        let transport = Arc::new(RusshTransport);
        let prompter = Arc::new(EventPrompter::new());

        let profiles = Arc::new(ProfileService::new(repo, Arc::clone(&secrets)));
        let secrets_for_state = Arc::clone(&secrets);
        let sessions = Arc::new(SessionService::new(
            transport,
            Arc::clone(&known_hosts),
            Arc::clone(&prompter) as Arc<dyn HostKeyPrompter>,
        ));
        let transfers = Arc::new(TransferService::new(
            Arc::clone(&sessions) as Arc<dyn harbor_core::application::SftpProvider>,
        ));
        let keys = Arc::new(KeyService::new(key_discovery));

        Ok(Self {
            profiles,
            sessions,
            transfers,
            keys,
            known_hosts,
            secrets: secrets_for_state,
            prompter,
            shells: Mutex::new(HashMap::new()),
            keychain_persistent,
        })
    }
}
