//! Server-metrics command: a single point-in-time snapshot of a session's
//! remote host (CPU, memory, disks, processes), gathered over SSH.

use tauri::State;

use harbor_core::domain::metrics::ServerMetrics;
use harbor_core::domain::session::SessionId;

use crate::error::CommandResult;
use crate::state::AppState;

/// Collect a fresh metrics snapshot for a connected session. The frontend polls
/// this on an interval while the Metrics panel is visible.
#[tauri::command]
pub async fn server_metrics(
    state: State<'_, AppState>,
    session_id: SessionId,
) -> CommandResult<ServerMetrics> {
    Ok(state.metrics.collect(session_id).await?)
}
