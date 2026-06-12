//! Key-management commands.

use tauri::State;

use harbor_core::domain::key::DiscoveredKey;

use crate::error::CommandResult;
use crate::state::AppState;

#[tauri::command]
pub async fn list_keys(state: State<'_, AppState>) -> CommandResult<Vec<DiscoveredKey>> {
    Ok(state.keys.list().await?)
}

#[tauri::command]
pub async fn inspect_key(state: State<'_, AppState>, path: String) -> CommandResult<DiscoveredKey> {
    Ok(state.keys.inspect(path).await?)
}
