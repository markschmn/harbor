//! Connection-manager commands.

use secrecy::SecretString;
use tauri::State;

use harbor_core::domain::profile::{ProfileDraft, ProfileId, ServerProfile};

use crate::error::CommandResult;
use crate::state::AppState;

#[tauri::command]
pub async fn list_profiles(state: State<'_, AppState>) -> CommandResult<Vec<ServerProfile>> {
    Ok(state.profiles.list().await?)
}

#[tauri::command]
pub async fn search_profiles(
    state: State<'_, AppState>,
    query: String,
) -> CommandResult<Vec<ServerProfile>> {
    Ok(state.profiles.search(&query).await?)
}

#[tauri::command]
pub async fn create_profile(
    state: State<'_, AppState>,
    draft: ProfileDraft,
) -> CommandResult<ServerProfile> {
    Ok(state.profiles.create(draft).await?)
}

#[tauri::command]
pub async fn update_profile(
    state: State<'_, AppState>,
    id: ProfileId,
    draft: ProfileDraft,
) -> CommandResult<ServerProfile> {
    Ok(state.profiles.update(id, draft).await?)
}

#[tauri::command]
pub async fn delete_profile(state: State<'_, AppState>, id: ProfileId) -> CommandResult<()> {
    state.profiles.delete(id).await?;
    Ok(())
}

#[tauri::command]
pub async fn toggle_favorite(
    state: State<'_, AppState>,
    id: ProfileId,
) -> CommandResult<ServerProfile> {
    Ok(state.profiles.toggle_favorite(id).await?)
}

#[tauri::command]
pub async fn set_profile_password(
    state: State<'_, AppState>,
    id: ProfileId,
    password: String,
) -> CommandResult<()> {
    state
        .profiles
        .set_password(id, SecretString::from(password))
        .await?;
    Ok(())
}

#[tauri::command]
pub async fn has_profile_password(
    state: State<'_, AppState>,
    id: ProfileId,
) -> CommandResult<bool> {
    Ok(state.profiles.has_password(id).await?)
}

#[tauri::command]
pub async fn clear_profile_password(
    state: State<'_, AppState>,
    id: ProfileId,
) -> CommandResult<()> {
    state.profiles.clear_password(id).await?;
    Ok(())
}
