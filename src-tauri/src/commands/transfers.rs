//! Transfer-manager commands.

use tauri::State;

use harbor_core::application::TransferRequest;
use harbor_core::domain::session::SessionId;
use harbor_core::domain::transfer::{TransferDirection, TransferId, TransferTask};

use crate::error::CommandResult;
use crate::state::AppState;

#[tauri::command]
pub async fn upload_file(
    state: State<'_, AppState>,
    session_id: SessionId,
    local_path: String,
    remote_path: String,
) -> CommandResult<TransferId> {
    let request = TransferRequest {
        session_id,
        direction: TransferDirection::Upload,
        source: local_path,
        destination: remote_path,
    };
    Ok(state.transfers.enqueue(request).await?)
}

#[tauri::command]
pub async fn download_file(
    state: State<'_, AppState>,
    session_id: SessionId,
    remote_path: String,
    local_path: String,
) -> CommandResult<TransferId> {
    let request = TransferRequest {
        session_id,
        direction: TransferDirection::Download,
        source: remote_path,
        destination: local_path,
    };
    Ok(state.transfers.enqueue(request).await?)
}

#[tauri::command]
pub async fn list_transfers(state: State<'_, AppState>) -> CommandResult<Vec<TransferTask>> {
    Ok(state.transfers.list().await)
}

#[tauri::command]
pub async fn cancel_transfer(state: State<'_, AppState>, id: TransferId) -> CommandResult<()> {
    state.transfers.cancel(id).await?;
    Ok(())
}

#[tauri::command]
pub async fn retry_transfer(
    state: State<'_, AppState>,
    id: TransferId,
) -> CommandResult<TransferId> {
    Ok(state.transfers.retry(id).await?)
}

#[tauri::command]
pub async fn clear_finished_transfers(state: State<'_, AppState>) -> CommandResult<()> {
    state.transfers.clear_finished().await?;
    Ok(())
}
