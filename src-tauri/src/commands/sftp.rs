//! Remote (SFTP) filesystem commands for the right pane of the file manager.

use tauri::State;

use harbor_core::domain::file::DirEntry;
use harbor_core::domain::session::SessionId;

use crate::error::CommandResult;
use crate::state::AppState;

#[tauri::command]
pub async fn list_remote_dir(
    state: State<'_, AppState>,
    session_id: SessionId,
    path: String,
) -> CommandResult<Vec<DirEntry>> {
    let sftp = state.sessions.sftp(session_id).await?;
    Ok(sftp.read_dir(&path).await?)
}

#[tauri::command]
pub async fn remote_home_dir(
    state: State<'_, AppState>,
    session_id: SessionId,
) -> CommandResult<String> {
    let sftp = state.sessions.sftp(session_id).await?;
    Ok(sftp.canonicalize(".").await?)
}

#[tauri::command]
pub async fn make_remote_dir(
    state: State<'_, AppState>,
    session_id: SessionId,
    path: String,
) -> CommandResult<()> {
    let sftp = state.sessions.sftp(session_id).await?;
    sftp.mkdir(&path).await?;
    Ok(())
}

#[tauri::command]
pub async fn remove_remote(
    state: State<'_, AppState>,
    session_id: SessionId,
    path: String,
    is_dir: bool,
) -> CommandResult<()> {
    let sftp = state.sessions.sftp(session_id).await?;
    if is_dir {
        sftp.remove_dir(&path).await?;
    } else {
        sftp.remove_file(&path).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn rename_remote(
    state: State<'_, AppState>,
    session_id: SessionId,
    from: String,
    to: String,
) -> CommandResult<()> {
    let sftp = state.sessions.sftp(session_id).await?;
    sftp.rename(&from, &to).await?;
    Ok(())
}
