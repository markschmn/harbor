//! Session and interactive-terminal commands.

use base64::Engine;
use secrecy::SecretString;
use serde::Deserialize;
use tauri::{AppHandle, Emitter, State};

use harbor_core::application::ports::{ConnectionParams, SecretRef, ShellEvent, ShellHandle, ShellInput};
use harbor_core::domain::auth::{AuthMethod, Credential};
use harbor_core::domain::host_key::TofuResolution;
use harbor_core::domain::profile::{ProfileId, ServerProfile};
use harbor_core::domain::session::{PtySize, SessionId, SessionInfo};

use crate::error::{CommandError, CommandResult};
use crate::state::AppState;

/// Event carrying a chunk of terminal output (data is base64-encoded bytes).
pub const EVENT_TERMINAL_DATA: &str = "harbor://terminal-data";
/// Event emitted when a shell closes.
pub const EVENT_TERMINAL_CLOSED: &str = "harbor://terminal-closed";

#[derive(Debug, Clone, serde::Serialize)]
struct TerminalDataPayload {
    session_id: String,
    data: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct TerminalClosedPayload {
    session_id: String,
    exit_code: Option<u32>,
}

/// Request body for [`connect`].
#[derive(Debug, Deserialize)]
pub struct ConnectRequest {
    pub profile_id: ProfileId,
    /// A password supplied interactively (for password auth).
    pub password: Option<String>,
    /// A passphrase to decrypt an encrypted private key.
    pub passphrase: Option<String>,
    /// Persist the supplied secret in the OS keychain for next time.
    #[serde(default)]
    pub remember_secret: bool,
}

#[tauri::command]
pub async fn connect(
    state: State<'_, AppState>,
    request: ConnectRequest,
) -> CommandResult<SessionInfo> {
    let profile = state.profiles.get(request.profile_id).await?;
    let credential = resolve_credential(&state, &profile, &request).await?;
    let params = ConnectionParams {
        host: profile.host.clone(),
        port: profile.port,
        username: profile.username.clone(),
        credential,
    };
    let info = state
        .sessions
        .connect(params, Some(profile.id), profile.name.clone())
        .await?;
    Ok(info)
}

/// Build the transient [`Credential`] for a connection, pulling from the request
/// or the keychain as appropriate and optionally persisting it.
async fn resolve_credential(
    state: &AppState,
    profile: &ServerProfile,
    request: &ConnectRequest,
) -> CommandResult<Credential> {
    match &profile.auth {
        AuthMethod::Agent => Ok(Credential::Agent),

        AuthMethod::Password => {
            if let Some(pw) = &request.password {
                if request.remember_secret {
                    let _ = state
                        .profiles
                        .set_password(profile.id, SecretString::from(pw.clone()))
                        .await;
                }
                Ok(Credential::Password(SecretString::from(pw.clone())))
            } else if let Some(stored) = state.profiles.get_password(profile.id).await? {
                Ok(Credential::Password(stored))
            } else {
                Err(CommandError::new(
                    "password_required",
                    "this profile uses password authentication; a password is required",
                ))
            }
        }

        AuthMethod::PublicKey { key_path, .. } => {
            let secret_ref = SecretRef::KeyPassphrase(key_path.to_string_lossy().into_owned());
            let passphrase = if let Some(p) = &request.passphrase {
                if request.remember_secret {
                    let _ = state
                        .secrets
                        .set(&secret_ref, SecretString::from(p.clone()))
                        .await;
                }
                Some(SecretString::from(p.clone()))
            } else {
                state.secrets.get(&secret_ref).await.unwrap_or(None)
            };
            Ok(Credential::PrivateKey {
                key_path: key_path.clone(),
                passphrase,
            })
        }
    }
}

#[tauri::command]
pub async fn disconnect(state: State<'_, AppState>, session_id: SessionId) -> CommandResult<()> {
    state.shells.lock().await.remove(&session_id);
    state.sessions.disconnect(session_id).await?;
    Ok(())
}

#[tauri::command]
pub async fn list_sessions(state: State<'_, AppState>) -> CommandResult<Vec<SessionInfo>> {
    Ok(state.sessions.list().await)
}

#[tauri::command]
pub async fn open_shell(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: SessionId,
    cols: u32,
    rows: u32,
) -> CommandResult<()> {
    let size = PtySize {
        cols,
        rows,
        ..Default::default()
    };
    let ShellHandle {
        input,
        mut output,
        ..
    } = state.sessions.open_shell(session_id, size).await?;

    state.shells.lock().await.insert(session_id, input);

    // Pump remote output to the frontend until the shell closes.
    tauri::async_runtime::spawn(async move {
        let engine = base64::engine::general_purpose::STANDARD;
        while let Some(event) = output.recv().await {
            match event {
                ShellEvent::Output(bytes) => {
                    let payload = TerminalDataPayload {
                        session_id: session_id.to_string(),
                        data: engine.encode(&bytes),
                    };
                    if app.emit(EVENT_TERMINAL_DATA, payload).is_err() {
                        break;
                    }
                }
                ShellEvent::Closed { exit_code } => {
                    let _ = app.emit(
                        EVENT_TERMINAL_CLOSED,
                        TerminalClosedPayload {
                            session_id: session_id.to_string(),
                            exit_code,
                        },
                    );
                    break;
                }
            }
        }
    });

    Ok(())
}

#[tauri::command]
pub async fn send_input(
    state: State<'_, AppState>,
    session_id: SessionId,
    data: String,
) -> CommandResult<()> {
    let sender = state.shells.lock().await.get(&session_id).cloned();
    match sender {
        Some(tx) => tx
            .send(ShellInput::Data(data.into_bytes()))
            .await
            .map_err(|_| CommandError::new("session_not_connected", "the shell has closed")),
        None => Err(CommandError::new(
            "session_not_connected",
            "no interactive shell is open for this session",
        )),
    }
}

#[tauri::command]
pub async fn resize_terminal(
    state: State<'_, AppState>,
    session_id: SessionId,
    cols: u32,
    rows: u32,
) -> CommandResult<()> {
    let sender = state.shells.lock().await.get(&session_id).cloned();
    if let Some(tx) = sender {
        let _ = tx
            .send(ShellInput::Resize(PtySize {
                cols,
                rows,
                ..Default::default()
            }))
            .await;
    }
    Ok(())
}

#[tauri::command]
pub async fn respond_host_key(
    state: State<'_, AppState>,
    request_id: String,
    resolution: TofuResolution,
) -> CommandResult<()> {
    state.prompter.respond(&request_id, resolution).await;
    Ok(())
}
