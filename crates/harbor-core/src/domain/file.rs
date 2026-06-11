//! File-system value objects shared by the local and remote file browsers.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// The kind of a directory entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    File,
    Directory,
    Symlink,
    Other,
}

/// A single entry in a directory listing, used for both local and remote
/// browsing so the UI can render them identically.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirEntry {
    /// File name only (no directory component).
    pub name: String,
    /// Full path as understood by the relevant filesystem.
    pub path: String,
    pub kind: FileKind,
    /// Size in bytes (0 for directories).
    pub size: u64,
    /// Unix permission bits, when available.
    pub permissions: Option<u32>,
    /// Last modification time, when available.
    #[serde(with = "time::serde::rfc3339::option")]
    pub modified: Option<OffsetDateTime>,
    /// For symlinks, the resolved target (best effort).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symlink_target: Option<String>,
}

impl DirEntry {
    pub fn is_dir(&self) -> bool {
        matches!(self.kind, FileKind::Directory)
    }

    /// Whether this entry is a "hidden" dotfile.
    pub fn is_hidden(&self) -> bool {
        self.name.starts_with('.')
    }

    /// Render Unix permission bits as an `rwxr-xr-x` style string.
    pub fn permission_string(&self) -> Option<String> {
        let mode = self.permissions?;
        let mut s = String::with_capacity(10);
        s.push(match self.kind {
            FileKind::Directory => 'd',
            FileKind::Symlink => 'l',
            _ => '-',
        });
        const FLAGS: [(u32, char); 9] = [
            (0o400, 'r'),
            (0o200, 'w'),
            (0o100, 'x'),
            (0o040, 'r'),
            (0o020, 'w'),
            (0o010, 'x'),
            (0o004, 'r'),
            (0o002, 'w'),
            (0o001, 'x'),
        ];
        for (bit, ch) in FLAGS {
            s.push(if mode & bit != 0 { ch } else { '-' });
        }
        Some(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(kind: FileKind, perms: Option<u32>) -> DirEntry {
        DirEntry {
            name: ".bashrc".into(),
            path: "/home/me/.bashrc".into(),
            kind,
            size: 10,
            permissions: perms,
            modified: None,
            symlink_target: None,
        }
    }

    #[test]
    fn hidden_detection() {
        assert!(entry(FileKind::File, None).is_hidden());
    }

    #[test]
    fn permission_string_rendering() {
        let e = entry(FileKind::File, Some(0o644));
        assert_eq!(e.permission_string().as_deref(), Some("-rw-r--r--"));
        let d = entry(FileKind::Directory, Some(0o755));
        assert_eq!(d.permission_string().as_deref(), Some("drwxr-xr-x"));
        assert_eq!(entry(FileKind::File, None).permission_string(), None);
    }
}
