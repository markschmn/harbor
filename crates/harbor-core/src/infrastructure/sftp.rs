//! The russh-sftp-backed [`SftpClient`].
//!
//! ## Why SFTP (not SCP)?
//!
//! Harbor uses **SFTP** for all file operations. SFTP is a stateful subsystem
//! of SSH that exposes real filesystem semantics — directory listings,
//! `stat`/`rename`/`mkdir`/`remove`, random access reads/writes, and resumable
//! ranged transfers. SCP, by contrast, is essentially a remote `cp` wrapped in
//! a shell pipe: it cannot list directories, has historically suffered from
//! parsing/quoting vulnerabilities (e.g. CVE-2019-6111), and OpenSSH itself now
//! recommends SFTP over the legacy SCP protocol. For a graphical dual-pane file
//! manager, SFTP is both the safer and the more capable choice.

use std::path::Path;

use async_trait::async_trait;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::FileType;
use time::OffsetDateTime;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::application::ports::{CancelFlag, ProgressFn, SftpClient};
use crate::domain::error::{HarborError, Result};
use crate::domain::file::{DirEntry, FileKind};

/// Chunk size used for streamed transfers; a good balance between syscall
/// overhead and progress-update granularity.
const TRANSFER_CHUNK: usize = 64 * 1024;

/// Wraps a russh-sftp [`SftpSession`] and adapts it to the [`SftpClient`] port.
pub struct RusshSftpClient {
    session: SftpSession,
}

impl std::fmt::Debug for RusshSftpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RusshSftpClient")
    }
}

impl RusshSftpClient {
    pub fn new(session: SftpSession) -> Self {
        Self { session }
    }
}

fn sftp_err(e: impl std::fmt::Display) -> HarborError {
    HarborError::Sftp(e.to_string())
}

fn io_err(path: &Path, e: std::io::Error) -> HarborError {
    HarborError::Io {
        path: path.to_path_buf(),
        source: e,
    }
}

fn map_file_type(ft: FileType) -> FileKind {
    match ft {
        FileType::Dir => FileKind::Directory,
        FileType::File => FileKind::File,
        FileType::Symlink => FileKind::Symlink,
        FileType::Other => FileKind::Other,
    }
}

/// Join a remote directory and a file name using `/` (SFTP always uses POSIX
/// paths regardless of the server OS).
fn join_remote(dir: &str, name: &str) -> String {
    if dir.is_empty() || dir == "/" {
        format!("/{name}")
    } else if dir.ends_with('/') {
        format!("{dir}{name}")
    } else {
        format!("{dir}/{name}")
    }
}

fn base_name(path: &str) -> String {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .to_string()
}

fn metadata_to_entry(name: String, path: String, meta: &russh_sftp::client::fs::Metadata) -> DirEntry {
    let modified = meta
        .modified()
        .ok()
        .map(OffsetDateTime::from);
    DirEntry {
        name,
        path,
        kind: map_file_type(meta.file_type()),
        size: meta.size.unwrap_or(0),
        permissions: meta.permissions,
        modified,
        symlink_target: None,
    }
}

#[async_trait]
impl SftpClient for RusshSftpClient {
    async fn read_dir(&self, path: &str) -> Result<Vec<DirEntry>> {
        let read = self.session.read_dir(path).await.map_err(sftp_err)?;
        let mut entries = Vec::new();
        for entry in read {
            let name = entry.file_name();
            if name == "." || name == ".." {
                continue;
            }
            let full = join_remote(path, &name);
            let meta = entry.metadata();
            entries.push(metadata_to_entry(name, full, &meta));
        }
        // Directories first, then case-insensitive by name.
        entries.sort_by(|a, b| {
            b.is_dir()
                .cmp(&a.is_dir())
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(entries)
    }

    async fn canonicalize(&self, path: &str) -> Result<String> {
        self.session.canonicalize(path).await.map_err(sftp_err)
    }

    async fn mkdir(&self, path: &str) -> Result<()> {
        self.session.create_dir(path).await.map_err(sftp_err)
    }

    async fn remove_file(&self, path: &str) -> Result<()> {
        self.session.remove_file(path).await.map_err(sftp_err)
    }

    async fn remove_dir(&self, path: &str) -> Result<()> {
        self.session.remove_dir(path).await.map_err(sftp_err)
    }

    async fn rename(&self, from: &str, to: &str) -> Result<()> {
        self.session.rename(from, to).await.map_err(sftp_err)
    }

    async fn stat(&self, path: &str) -> Result<DirEntry> {
        let meta = self.session.metadata(path).await.map_err(sftp_err)?;
        Ok(metadata_to_entry(base_name(path), path.to_string(), &meta))
    }

    async fn download(
        &self,
        remote: &str,
        local: &Path,
        progress: ProgressFn,
        cancel: CancelFlag,
    ) -> Result<u64> {
        use std::sync::atomic::Ordering;

        let mut remote_file = self.session.open(remote).await.map_err(sftp_err)?;
        let total = remote_file
            .metadata()
            .await
            .ok()
            .and_then(|m| m.size)
            .unwrap_or(0);

        if let Some(parent) = local.parent() {
            if !parent.as_os_str().is_empty() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| io_err(parent, e))?;
            }
        }
        let mut out = tokio::fs::File::create(local)
            .await
            .map_err(|e| io_err(local, e))?;

        let mut buf = vec![0u8; TRANSFER_CHUNK];
        let mut transferred = 0u64;
        loop {
            if cancel.load(Ordering::SeqCst) {
                drop(out);
                let _ = tokio::fs::remove_file(local).await; // clean partial file
                return Err(HarborError::Cancelled);
            }
            let n = remote_file.read(&mut buf).await.map_err(sftp_err)?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n]).await.map_err(|e| io_err(local, e))?;
            transferred += n as u64;
            progress(transferred, total.max(transferred));
        }
        out.flush().await.map_err(|e| io_err(local, e))?;
        progress(transferred, transferred);
        Ok(transferred)
    }

    async fn upload(
        &self,
        local: &Path,
        remote: &str,
        progress: ProgressFn,
        cancel: CancelFlag,
    ) -> Result<u64> {
        use std::sync::atomic::Ordering;

        let mut input = tokio::fs::File::open(local)
            .await
            .map_err(|e| io_err(local, e))?;
        let total = input.metadata().await.map(|m| m.len()).unwrap_or(0);

        let mut remote_file = self.session.create(remote).await.map_err(sftp_err)?;

        let mut buf = vec![0u8; TRANSFER_CHUNK];
        let mut transferred = 0u64;
        loop {
            if cancel.load(Ordering::SeqCst) {
                let _ = remote_file.shutdown().await;
                let _ = self.session.remove_file(remote).await; // clean partial file
                return Err(HarborError::Cancelled);
            }
            let n = input.read(&mut buf).await.map_err(|e| io_err(local, e))?;
            if n == 0 {
                break;
            }
            remote_file.write_all(&buf[..n]).await.map_err(sftp_err)?;
            transferred += n as u64;
            progress(transferred, total.max(transferred));
        }
        remote_file.flush().await.map_err(sftp_err)?;
        remote_file.shutdown().await.map_err(sftp_err)?;
        progress(transferred, transferred);
        Ok(transferred)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_remote_handles_separators() {
        assert_eq!(join_remote("/home/me", "file"), "/home/me/file");
        assert_eq!(join_remote("/home/me/", "file"), "/home/me/file");
        assert_eq!(join_remote("/", "file"), "/file");
        assert_eq!(join_remote("", "file"), "/file");
    }

    #[test]
    fn base_name_extraction() {
        assert_eq!(base_name("/home/me/file.txt"), "file.txt");
        assert_eq!(base_name("/home/me/dir/"), "dir");
        assert_eq!(base_name("file"), "file");
    }

    #[test]
    fn file_type_mapping() {
        assert_eq!(map_file_type(FileType::Dir), FileKind::Directory);
        assert_eq!(map_file_type(FileType::File), FileKind::File);
        assert_eq!(map_file_type(FileType::Symlink), FileKind::Symlink);
        assert_eq!(map_file_type(FileType::Other), FileKind::Other);
    }
}
