//! Harbor — Tauri application entry point and command wiring.
//!
//! This crate is the **presentation** layer. It owns no business logic; it
//! exposes `harbor-core` services to the React frontend via Tauri commands and
//! bridges core event streams (terminal output, transfer progress, host-key
//! prompts) to the webview's event system.

mod commands;
mod error;
mod state;

use serde::Serialize;
use tauri::{Emitter, Manager};

use harbor_core::application::TransferEvent;
use harbor_core::domain::transfer::{TransferId, TransferProgress, TransferState, TransferTask};

use state::AppState;

/// Event carrying transfer-manager updates to the UI.
pub const EVENT_TRANSFER: &str = "harbor://transfer-event";

/// Serialisable mirror of [`TransferEvent`] for the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum TransferEventDto {
    Added { task: TransferTask },
    Progress { progress: TransferProgress },
    StateChanged { id: TransferId, state: TransferState },
}

impl From<TransferEvent> for TransferEventDto {
    fn from(e: TransferEvent) -> Self {
        match e {
            TransferEvent::Added(task) => TransferEventDto::Added { task },
            TransferEvent::Progress(progress) => TransferEventDto::Progress { progress },
            TransferEvent::StateChanged { id, state } => {
                TransferEventDto::StateChanged { id, state }
            }
        }
    }
}

fn init_tracing() {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};
    let filter = EnvFilter::try_from_env("HARBOR_LOG")
        .or_else(|_| EnvFilter::try_new("info,harbor=debug,harbor_core=debug"))
        .unwrap_or_default();
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer())
        .try_init();
}

/// Build and run the Harbor desktop application.
pub fn run() {
    init_tracing();

    let app_state = tauri::async_runtime::block_on(AppState::new())
        .expect("failed to initialise Harbor application state");

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_os::init())
        .manage(app_state)
        .setup(|app| {
            let handle = app.handle().clone();
            let state = app.state::<AppState>();

            // Give the host-key prompter a handle so it can emit prompts.
            let prompter = std::sync::Arc::clone(&state.prompter);
            let prompter_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                prompter.set_app_handle(prompter_handle).await;
            });

            // Forward transfer-manager events to the frontend.
            let mut rx = state.transfers.subscribe();
            let transfer_handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    match rx.recv().await {
                        Ok(event) => {
                            let _ = transfer_handle
                                .emit(EVENT_TRANSFER, TransferEventDto::from(event));
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(_) => break,
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_info,
            commands::app::forget_host,
            commands::profiles::list_profiles,
            commands::profiles::search_profiles,
            commands::profiles::create_profile,
            commands::profiles::update_profile,
            commands::profiles::delete_profile,
            commands::profiles::toggle_favorite,
            commands::profiles::set_profile_password,
            commands::profiles::has_profile_password,
            commands::profiles::clear_profile_password,
            commands::sessions::connect,
            commands::sessions::disconnect,
            commands::sessions::list_sessions,
            commands::sessions::open_shell,
            commands::sessions::send_input,
            commands::sessions::resize_terminal,
            commands::sessions::respond_host_key,
            commands::sftp::list_remote_dir,
            commands::sftp::remote_home_dir,
            commands::sftp::make_remote_dir,
            commands::sftp::remove_remote,
            commands::sftp::rename_remote,
            commands::local_fs::list_local_dir,
            commands::local_fs::local_home_dir,
            commands::local_fs::local_parent_dir,
            commands::local_fs::make_local_dir,
            commands::local_fs::remove_local,
            commands::local_fs::rename_local,
            commands::transfers::upload_file,
            commands::transfers::download_file,
            commands::transfers::list_transfers,
            commands::transfers::cancel_transfer,
            commands::transfers::retry_transfer,
            commands::transfers::clear_finished_transfers,
            commands::keys::list_keys,
            commands::keys::inspect_key,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the Harbor application");
}
