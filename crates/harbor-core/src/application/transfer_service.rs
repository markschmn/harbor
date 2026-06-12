//! Transfer-manager use cases: a concurrent, observable queue of SFTP uploads
//! and downloads with history, retry and cancellation.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use time::OffsetDateTime;
use tokio::sync::{broadcast, Mutex, Semaphore};

use crate::domain::error::{HarborError, Result};
use crate::domain::session::SessionId;
use crate::domain::transfer::{
    TransferDirection, TransferId, TransferProgress, TransferState, TransferTask,
};

use super::ports::{ProgressFn, SftpClient};

/// Resolves an SFTP client for a session. Implemented by
/// [`SessionService`](super::SessionService); abstracted so the transfer
/// manager can be tested without a real connection.
#[async_trait]
pub trait SftpProvider: Send + Sync {
    async fn sftp_for(&self, id: SessionId) -> Result<Arc<dyn SftpClient>>;
}

/// A request to enqueue a transfer.
#[derive(Debug, Clone)]
pub struct TransferRequest {
    pub session_id: SessionId,
    pub direction: TransferDirection,
    /// Source path (local for upload, remote for download).
    pub source: String,
    /// Destination path (remote for upload, local for download).
    pub destination: String,
}

/// Events emitted as transfers progress. Forwarded to the UI by the
/// presentation layer.
#[derive(Debug, Clone)]
pub enum TransferEvent {
    Added(TransferTask),
    Progress(TransferProgress),
    StateChanged {
        id: TransferId,
        state: TransferState,
    },
}

/// The maximum number of transfers that run at the same time.
pub const DEFAULT_MAX_CONCURRENCY: usize = 3;

/// Emit a progress event at most this often (in bytes) to avoid flooding the
/// UI event channel on fast links.
const PROGRESS_EVERY_BYTES: u64 = 64 * 1024;

type TaskMap = Arc<Mutex<HashMap<TransferId, TransferTask>>>;

/// Drives concurrent SFTP transfers.
pub struct TransferService {
    provider: Arc<dyn SftpProvider>,
    tasks: TaskMap,
    cancels: Arc<Mutex<HashMap<TransferId, Arc<AtomicBool>>>>,
    semaphore: Arc<Semaphore>,
    events: broadcast::Sender<TransferEvent>,
}

impl std::fmt::Debug for TransferService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TransferService")
    }
}

impl TransferService {
    pub fn new(provider: Arc<dyn SftpProvider>) -> Self {
        Self::with_concurrency(provider, DEFAULT_MAX_CONCURRENCY)
    }

    pub fn with_concurrency(provider: Arc<dyn SftpProvider>, max: usize) -> Self {
        let (events, _) = broadcast::channel(1024);
        Self {
            provider,
            tasks: Arc::new(Mutex::new(HashMap::new())),
            cancels: Arc::new(Mutex::new(HashMap::new())),
            semaphore: Arc::new(Semaphore::new(max.max(1))),
            events,
        }
    }

    /// Subscribe to the transfer event stream.
    pub fn subscribe(&self) -> broadcast::Receiver<TransferEvent> {
        self.events.subscribe()
    }

    /// All transfers, newest first (acts as the transfer history too).
    pub async fn list(&self) -> Vec<TransferTask> {
        let mut v: Vec<_> = self.tasks.lock().await.values().cloned().collect();
        v.sort_by_key(|t| std::cmp::Reverse(t.created_at));
        v
    }

    pub async fn get(&self, id: TransferId) -> Option<TransferTask> {
        self.tasks.lock().await.get(&id).cloned()
    }

    /// Enqueue a transfer and start driving it (subject to the concurrency
    /// limit). Returns the new task id immediately.
    pub async fn enqueue(self: &Arc<Self>, request: TransferRequest) -> Result<TransferId> {
        let id = TransferId::new();
        let file_name = base_name(match request.direction {
            TransferDirection::Upload => &request.source,
            TransferDirection::Download => &request.destination,
        });
        let task = TransferTask {
            id,
            session_id: request.session_id,
            direction: request.direction,
            source: request.source.clone(),
            destination: request.destination.clone(),
            file_name,
            state: TransferState::Queued,
            total_bytes: 0,
            transferred_bytes: 0,
            created_at: OffsetDateTime::now_utc(),
            started_at: None,
            finished_at: None,
        };
        self.tasks.lock().await.insert(id, task.clone());
        let _ = self.events.send(TransferEvent::Added(task));

        let this = Arc::clone(self);
        tokio::spawn(async move {
            this.drive(id, request).await;
        });
        Ok(id)
    }

    /// Cancel an in-flight or queued transfer.
    pub async fn cancel(&self, id: TransferId) -> Result<()> {
        if let Some(flag) = self.cancels.lock().await.get(&id) {
            flag.store(true, Ordering::SeqCst);
        }
        // Queued-but-not-started tasks get marked immediately; running tasks
        // will observe the flag and finish as Cancelled.
        let mut guard = self.tasks.lock().await;
        if let Some(t) = guard.get_mut(&id) {
            if matches!(t.state, TransferState::Queued | TransferState::Paused) {
                t.state = TransferState::Cancelled;
                t.finished_at = Some(OffsetDateTime::now_utc());
                let _ = self.events.send(TransferEvent::StateChanged {
                    id,
                    state: t.state.clone(),
                });
            }
        }
        Ok(())
    }

    /// Retry a failed or cancelled transfer by re-enqueuing it.
    pub async fn retry(self: &Arc<Self>, id: TransferId) -> Result<TransferId> {
        let task = self
            .tasks
            .lock()
            .await
            .get(&id)
            .cloned()
            .ok_or_else(|| HarborError::NotFound(format!("transfer {id}")))?;
        if !task.state.is_retryable() {
            return Err(HarborError::validation(
                "only failed or cancelled transfers can be retried",
            ));
        }
        self.enqueue(TransferRequest {
            session_id: task.session_id,
            direction: task.direction,
            source: task.source,
            destination: task.destination,
        })
        .await
    }

    /// Remove all terminal (completed/failed/cancelled) transfers from history.
    pub async fn clear_finished(&self) -> Result<()> {
        self.tasks
            .lock()
            .await
            .retain(|_, t| !t.state.is_terminal());
        Ok(())
    }

    // ----- internals ---------------------------------------------------------

    async fn set_state(&self, id: TransferId, state: TransferState) {
        let mut guard = self.tasks.lock().await;
        if let Some(t) = guard.get_mut(&id) {
            if matches!(state, TransferState::Active) {
                t.started_at.get_or_insert_with(OffsetDateTime::now_utc);
            }
            if state.is_terminal() {
                t.finished_at = Some(OffsetDateTime::now_utc());
            }
            t.state = state.clone();
        }
        let _ = self.events.send(TransferEvent::StateChanged { id, state });
    }

    async fn drive(self: Arc<Self>, id: TransferId, request: TransferRequest) {
        // Respect a cancellation that arrived while still queued.
        if matches!(
            self.tasks.lock().await.get(&id).map(|t| t.state.clone()),
            Some(TransferState::Cancelled)
        ) {
            return;
        }

        let permit = match self.semaphore.clone().acquire_owned().await {
            Ok(p) => p,
            Err(_) => return,
        };

        // Cancellation may also have arrived while we waited for a permit.
        if matches!(
            self.tasks.lock().await.get(&id).map(|t| t.state.clone()),
            Some(TransferState::Cancelled)
        ) {
            return;
        }

        let cancel = Arc::new(AtomicBool::new(false));
        self.cancels.lock().await.insert(id, Arc::clone(&cancel));

        self.set_state(id, TransferState::Active).await;

        let result = self.run_transfer(id, &request, Arc::clone(&cancel)).await;
        drop(permit);
        self.cancels.lock().await.remove(&id);

        let final_state = match result {
            Ok(_) => TransferState::Completed,
            Err(HarborError::Cancelled) => TransferState::Cancelled,
            Err(e) => TransferState::Failed {
                error: e.to_string(),
            },
        };
        self.set_state(id, final_state).await;
    }

    async fn run_transfer(
        &self,
        id: TransferId,
        request: &TransferRequest,
        cancel: Arc<AtomicBool>,
    ) -> Result<u64> {
        let sftp = self.provider.sftp_for(request.session_id).await?;

        let progress = self.make_progress_callback(id);

        match request.direction {
            TransferDirection::Upload => {
                let local = PathBuf::from(&request.source);
                sftp.upload(&local, &request.destination, progress, cancel)
                    .await
            }
            TransferDirection::Download => {
                let local = PathBuf::from(&request.destination);
                sftp.download(&request.source, &local, progress, cancel)
                    .await
            }
        }
    }

    /// Build a progress callback that updates the task and emits throttled
    /// [`TransferEvent::Progress`] events with a smoothed throughput figure.
    fn make_progress_callback(&self, id: TransferId) -> ProgressFn {
        let tasks = Arc::clone(&self.tasks);
        let events = self.events.clone();
        let start = Instant::now();
        let last_emitted = Arc::new(AtomicU64::new(0));

        Arc::new(move |transferred: u64, total: u64| {
            // Update the authoritative task record. `try_lock` keeps the hot
            // path non-blocking; a missed update is corrected by the next call.
            if let Ok(mut guard) = tasks.try_lock() {
                if let Some(t) = guard.get_mut(&id) {
                    t.transferred_bytes = transferred;
                    if total > 0 {
                        t.total_bytes = total;
                    }
                }
            }

            let prev = last_emitted.load(Ordering::Relaxed);
            let crossed = transferred.saturating_sub(prev) >= PROGRESS_EVERY_BYTES;
            let finished = total > 0 && transferred >= total;
            if crossed || finished {
                last_emitted.store(transferred, Ordering::Relaxed);
                let secs = start.elapsed().as_secs_f64().max(0.001);
                let bps = (transferred as f64 / secs) as u64;
                let _ = events.send(TransferEvent::Progress(TransferProgress {
                    id,
                    transferred_bytes: transferred,
                    total_bytes: total,
                    bytes_per_second: bps,
                }));
            }
        })
    }
}

/// Extract the final path component (works for both `/` and `\` separators).
fn base_name(path: &str) -> String {
    path.rsplit(['/', '\\'])
        .find(|s| !s.is_empty())
        .unwrap_or(path)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::ports::CancelFlag;
    use crate::domain::file::DirEntry;
    use std::path::Path;
    use std::time::Duration;

    /// A fake SFTP client that "transfers" a fixed number of bytes in chunks,
    /// honouring the cancel flag, so we can exercise the queue deterministically.
    struct FakeSftp {
        total: u64,
        chunk: u64,
        fail: bool,
    }

    #[async_trait]
    impl SftpClient for FakeSftp {
        async fn read_dir(&self, _path: &str) -> Result<Vec<DirEntry>> {
            Ok(vec![])
        }
        async fn canonicalize(&self, path: &str) -> Result<String> {
            Ok(path.to_string())
        }
        async fn mkdir(&self, _path: &str) -> Result<()> {
            Ok(())
        }
        async fn remove_file(&self, _path: &str) -> Result<()> {
            Ok(())
        }
        async fn remove_dir(&self, _path: &str) -> Result<()> {
            Ok(())
        }
        async fn rename(&self, _from: &str, _to: &str) -> Result<()> {
            Ok(())
        }
        async fn stat(&self, _path: &str) -> Result<DirEntry> {
            Err(HarborError::Sftp("no stat".into()))
        }
        async fn download(
            &self,
            _remote: &str,
            _local: &Path,
            progress: ProgressFn,
            cancel: CancelFlag,
        ) -> Result<u64> {
            self.run(progress, cancel).await
        }
        async fn upload(
            &self,
            _local: &Path,
            _remote: &str,
            progress: ProgressFn,
            cancel: CancelFlag,
        ) -> Result<u64> {
            self.run(progress, cancel).await
        }
    }

    impl FakeSftp {
        async fn run(&self, progress: ProgressFn, cancel: CancelFlag) -> Result<u64> {
            if self.fail {
                return Err(HarborError::Sftp("boom".into()));
            }
            let mut done = 0u64;
            while done < self.total {
                if cancel.load(Ordering::SeqCst) {
                    return Err(HarborError::Cancelled);
                }
                done = (done + self.chunk).min(self.total);
                progress(done, self.total);
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            Ok(done)
        }
    }

    struct FakeProvider {
        sftp: Arc<FakeSftp>,
    }

    #[async_trait]
    impl SftpProvider for FakeProvider {
        async fn sftp_for(&self, _id: SessionId) -> Result<Arc<dyn SftpClient>> {
            Ok(self.sftp.clone() as Arc<dyn SftpClient>)
        }
    }

    fn service(total: u64, fail: bool) -> Arc<TransferService> {
        Arc::new(TransferService::with_concurrency(
            Arc::new(FakeProvider {
                sftp: Arc::new(FakeSftp {
                    total,
                    chunk: 100 * 1024,
                    fail,
                }),
            }),
            2,
        ))
    }

    fn request() -> TransferRequest {
        TransferRequest {
            session_id: SessionId::new(),
            direction: TransferDirection::Upload,
            source: "/local/file.bin".into(),
            destination: "/remote/file.bin".into(),
        }
    }

    async fn wait_terminal(svc: &TransferService, id: TransferId) -> TransferState {
        for _ in 0..200 {
            if let Some(t) = svc.get(id).await {
                if t.state.is_terminal() {
                    return t.state;
                }
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("transfer did not finish");
    }

    #[tokio::test]
    async fn transfer_completes_and_reports_progress() {
        let svc = service(512 * 1024, false);
        let mut rx = svc.subscribe();
        let id = svc.enqueue(request()).await.unwrap();

        assert_eq!(wait_terminal(&svc, id).await, TransferState::Completed);
        let task = svc.get(id).await.unwrap();
        assert_eq!(task.transferred_bytes, 512 * 1024);
        assert_eq!(task.progress(), 1.0);

        // We should have observed at least one progress event.
        let mut saw_progress = false;
        while let Ok(ev) = rx.try_recv() {
            if matches!(ev, TransferEvent::Progress(_)) {
                saw_progress = true;
            }
        }
        assert!(saw_progress);
    }

    #[tokio::test]
    async fn failed_transfer_can_be_retried() {
        let svc = service(256 * 1024, true);
        let id = svc.enqueue(request()).await.unwrap();
        assert!(matches!(
            wait_terminal(&svc, id).await,
            TransferState::Failed { .. }
        ));

        let retry_id = svc.retry(id).await.unwrap();
        assert_ne!(retry_id, id);
        // The fake always fails, but the retry must run and reach a terminal state.
        assert!(matches!(
            wait_terminal(&svc, retry_id).await,
            TransferState::Failed { .. }
        ));
    }

    #[tokio::test]
    async fn cancel_stops_a_running_transfer() {
        let svc = service(50 * 1024 * 1024, false); // large enough to still be running
        let id = svc.enqueue(request()).await.unwrap();
        // Let it start.
        tokio::time::sleep(Duration::from_millis(20)).await;
        svc.cancel(id).await.unwrap();
        assert_eq!(wait_terminal(&svc, id).await, TransferState::Cancelled);
    }

    #[tokio::test]
    async fn clear_finished_removes_terminal_tasks() {
        let svc = service(64 * 1024, false);
        let id = svc.enqueue(request()).await.unwrap();
        wait_terminal(&svc, id).await;
        assert_eq!(svc.list().await.len(), 1);
        svc.clear_finished().await.unwrap();
        assert!(svc.list().await.is_empty());
    }
}
