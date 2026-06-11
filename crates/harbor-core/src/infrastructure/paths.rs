//! Resolves the per-user directories Harbor uses for configuration and data.
//!
//! Follows platform conventions via the `dirs` crate:
//! * Linux:   `~/.config/harbor`
//! * macOS:   `~/Library/Application Support/harbor`
//! * Windows: `%APPDATA%\harbor`
//!
//! A `HARBOR_CONFIG_DIR` environment variable overrides everything, which keeps
//! tests hermetic and lets power users relocate their data.

use std::path::PathBuf;

use crate::domain::error::{HarborError, Result};

/// Environment variable that, when set, overrides the config directory.
pub const CONFIG_DIR_ENV: &str = "HARBOR_CONFIG_DIR";

/// The directory that holds Harbor's configuration and profile data.
pub fn config_dir() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os(CONFIG_DIR_ENV) {
        return Ok(PathBuf::from(dir));
    }
    let base = dirs::config_dir().ok_or_else(|| {
        HarborError::Storage("could not determine the user config directory".into())
    })?;
    Ok(base.join("harbor"))
}

/// The user's `~/.ssh` directory, used for key discovery and `known_hosts`.
pub fn ssh_dir() -> Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| HarborError::Storage("could not determine the home directory".into()))?;
    Ok(home.join(".ssh"))
}

/// The `known_hosts` file Harbor reads and writes. Harbor uses the standard
/// OpenSSH location so trust decisions are shared with the system `ssh` client.
pub fn known_hosts_path() -> Result<PathBuf> {
    Ok(ssh_dir()?.join("known_hosts"))
}

/// Ensure a directory exists, creating it (and parents) if needed.
pub fn ensure_dir(path: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(path).map_err(|e| HarborError::Io {
        path: path.to_path_buf(),
        source: e,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_override_is_respected() {
        // SAFETY: single-threaded test; we set and read our own variable.
        std::env::set_var(CONFIG_DIR_ENV, "/tmp/harbor-test-xyz");
        assert_eq!(config_dir().unwrap(), PathBuf::from("/tmp/harbor-test-xyz"));
        std::env::remove_var(CONFIG_DIR_ENV);
    }
}
