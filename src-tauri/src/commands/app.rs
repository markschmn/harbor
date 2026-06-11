//! Miscellaneous application-level commands.

use serde::Serialize;
use tauri::State;

use crate::error::CommandResult;
use crate::state::AppState;

/// Information the UI needs about the running app/environment.
#[derive(Debug, Serialize)]
pub struct AppInfo {
    /// App version from Cargo.
    pub version: String,
    /// Whether secrets persist in the OS keychain (vs. session-only fallback).
    pub keychain_persistent: bool,
    /// Host platform, for UI hints (path separators etc.).
    pub platform: String,
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        keychain_persistent: state.keychain_persistent,
        platform: std::env::consts::OS.to_string(),
    }
}

/// Remove all trusted host-key entries for a host (the "forget host" action).
#[tauri::command]
pub async fn forget_host(
    state: State<'_, AppState>,
    host: String,
    port: u16,
) -> CommandResult<()> {
    state.known_hosts.forget(&host, port).await?;
    Ok(())
}
