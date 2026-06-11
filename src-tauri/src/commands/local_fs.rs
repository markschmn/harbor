//! Local filesystem browsing for the left pane of the file manager.

use std::path::{Path, PathBuf};

use time::OffsetDateTime;

use harbor_core::domain::file::{DirEntry, FileKind};

use crate::error::{CommandError, CommandResult};

fn map_err(path: &Path, e: std::io::Error) -> CommandError {
    CommandError::new("io", format!("{}: {e}", path.display()))
}

fn entry_from(path: &Path) -> Option<DirEntry> {
    let name = path.file_name()?.to_string_lossy().into_owned();
    // `symlink_metadata` so symlinks are reported as such, not followed.
    let meta = std::fs::symlink_metadata(path).ok()?;
    let kind = if meta.file_type().is_symlink() {
        FileKind::Symlink
    } else if meta.is_dir() {
        FileKind::Directory
    } else if meta.is_file() {
        FileKind::File
    } else {
        FileKind::Other
    };

    #[cfg(unix)]
    let permissions = {
        use std::os::unix::fs::PermissionsExt;
        Some(meta.permissions().mode())
    };
    #[cfg(not(unix))]
    let permissions = None;

    let modified = meta.modified().ok().map(OffsetDateTime::from);
    let symlink_target = if matches!(kind, FileKind::Symlink) {
        std::fs::read_link(path)
            .ok()
            .map(|p| p.to_string_lossy().into_owned())
    } else {
        None
    };

    Some(DirEntry {
        name,
        path: path.to_string_lossy().into_owned(),
        kind,
        size: if meta.is_dir() { 0 } else { meta.len() },
        permissions,
        modified,
        symlink_target,
    })
}

#[tauri::command]
pub fn list_local_dir(path: String) -> CommandResult<Vec<DirEntry>> {
    let dir = PathBuf::from(&path);
    let read = std::fs::read_dir(&dir).map_err(|e| map_err(&dir, e))?;

    let mut entries: Vec<DirEntry> = read
        .flatten()
        .filter_map(|e| entry_from(&e.path()))
        .collect();

    entries.sort_by(|a, b| {
        b.is_dir()
            .cmp(&a.is_dir())
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

#[tauri::command]
pub fn local_home_dir() -> CommandResult<String> {
    dirs_home()
        .map(|p| p.to_string_lossy().into_owned())
        .ok_or_else(|| CommandError::new("io", "could not determine the home directory"))
}

#[tauri::command]
pub fn local_parent_dir(path: String) -> CommandResult<Option<String>> {
    Ok(PathBuf::from(path)
        .parent()
        .map(|p| p.to_string_lossy().into_owned()))
}

#[tauri::command]
pub fn make_local_dir(path: String) -> CommandResult<()> {
    let p = PathBuf::from(&path);
    std::fs::create_dir(&p).map_err(|e| map_err(&p, e))
}

#[tauri::command]
pub fn remove_local(path: String, is_dir: bool) -> CommandResult<()> {
    let p = PathBuf::from(&path);
    let result = if is_dir {
        std::fs::remove_dir_all(&p)
    } else {
        std::fs::remove_file(&p)
    };
    result.map_err(|e| map_err(&p, e))
}

#[tauri::command]
pub fn rename_local(from: String, to: String) -> CommandResult<()> {
    let from_p = PathBuf::from(&from);
    let to_p = PathBuf::from(&to);
    std::fs::rename(&from_p, &to_p).map_err(|e| map_err(&from_p, e))
}

fn dirs_home() -> Option<PathBuf> {
    #[cfg(unix)]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }
    #[cfg(not(any(unix, windows)))]
    {
        None
    }
}
