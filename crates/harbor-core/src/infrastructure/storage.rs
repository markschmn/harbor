//! TOML-backed [`ProfileRepository`].
//!
//! All profiles live in a single `profiles.toml` document inside Harbor's
//! config directory. Writes are atomic (write-to-temp then rename) and, on Unix,
//! the file is created with `0600` permissions. The file never contains secret
//! material — passwords and passphrases live in the OS keychain.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::application::ports::ProfileRepository;
use crate::domain::error::{HarborError, Result};
use crate::domain::profile::{ProfileId, ServerProfile};

use super::paths;

/// On-disk document wrapping the list of profiles.
#[derive(Debug, Default, Serialize, Deserialize)]
struct ProfilesDocument {
    #[serde(default, rename = "profile")]
    profiles: Vec<ServerProfile>,
}

/// Persists profiles as a TOML file.
#[derive(Debug)]
pub struct TomlProfileRepository {
    path: PathBuf,
    /// Serialises read-modify-write cycles so concurrent upserts don't clobber.
    write_lock: Mutex<()>,
}

impl TomlProfileRepository {
    /// Use the default location (`<config_dir>/profiles.toml`).
    pub fn with_default_location() -> Result<Self> {
        let dir = paths::config_dir()?;
        paths::ensure_dir(&dir)?;
        Ok(Self::new(dir.join("profiles.toml")))
    }

    /// Use an explicit file path (used by tests).
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            write_lock: Mutex::new(()),
        }
    }

    fn load(&self) -> Result<ProfilesDocument> {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => toml::from_str(&text).map_err(HarborError::from),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ProfilesDocument::default()),
            Err(e) => Err(HarborError::Io {
                path: self.path.clone(),
                source: e,
            }),
        }
    }

    fn store(&self, doc: &ProfilesDocument) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            paths::ensure_dir(parent)?;
        }
        let text = toml::to_string_pretty(doc)?;
        write_atomic(&self.path, text.as_bytes())
    }
}

#[async_trait]
impl ProfileRepository for TomlProfileRepository {
    async fn list(&self) -> Result<Vec<ServerProfile>> {
        Ok(self.load()?.profiles)
    }

    async fn get(&self, id: ProfileId) -> Result<ServerProfile> {
        self.load()?
            .profiles
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| HarborError::NotFound(format!("profile {id}")))
    }

    async fn upsert(&self, profile: &ServerProfile) -> Result<()> {
        profile.validate()?;
        let _guard = self.write_lock.lock().unwrap();
        let mut doc = self.load()?;
        match doc.profiles.iter_mut().find(|p| p.id == profile.id) {
            Some(existing) => *existing = profile.clone(),
            None => doc.profiles.push(profile.clone()),
        }
        self.store(&doc)
    }

    async fn delete(&self, id: ProfileId) -> Result<()> {
        let _guard = self.write_lock.lock().unwrap();
        let mut doc = self.load()?;
        let before = doc.profiles.len();
        doc.profiles.retain(|p| p.id != id);
        if doc.profiles.len() == before {
            return Err(HarborError::NotFound(format!("profile {id}")));
        }
        self.store(&doc)
    }
}

/// Write `bytes` to `path` atomically, restricting permissions on Unix.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;

    let tmp = path.with_extension("toml.tmp");
    let mut file = std::fs::File::create(&tmp).map_err(|e| HarborError::Io {
        path: tmp.clone(),
        source: e,
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        file.set_permissions(perms).map_err(|e| HarborError::Io {
            path: tmp.clone(),
            source: e,
        })?;
    }

    file.write_all(bytes).map_err(|e| HarborError::Io {
        path: tmp.clone(),
        source: e,
    })?;
    file.sync_all().map_err(|e| HarborError::Io {
        path: tmp.clone(),
        source: e,
    })?;
    drop(file);

    std::fs::rename(&tmp, path).map_err(|e| HarborError::Io {
        path: path.to_path_buf(),
        source: e,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::profile::ProfileDraft;
    use time::OffsetDateTime;

    fn repo() -> (TomlProfileRepository, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let repo = TomlProfileRepository::new(dir.path().join("profiles.toml"));
        (repo, dir)
    }

    fn profile(name: &str) -> ServerProfile {
        ServerProfile::from_draft(
            ProfileDraft {
                name: name.into(),
                host: "h.example.com".into(),
                username: "me".into(),
                ..Default::default()
            },
            OffsetDateTime::UNIX_EPOCH,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn missing_file_yields_empty_list() {
        let (repo, _d) = repo();
        assert!(repo.list().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn upsert_get_update_delete_round_trip() {
        let (repo, _d) = repo();
        let mut p = profile("alpha");
        repo.upsert(&p).await.unwrap();
        assert_eq!(repo.get(p.id).await.unwrap().name, "alpha");

        p.name = "alpha2".into();
        repo.upsert(&p).await.unwrap(); // update in place, not duplicate
        assert_eq!(repo.list().await.unwrap().len(), 1);
        assert_eq!(repo.get(p.id).await.unwrap().name, "alpha2");

        repo.delete(p.id).await.unwrap();
        assert!(repo.get(p.id).await.unwrap_err().code() == "not_found");
        assert!(repo.delete(p.id).await.is_err());
    }

    #[tokio::test]
    async fn data_survives_a_fresh_repository_instance() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("profiles.toml");
        {
            let repo = TomlProfileRepository::new(&path);
            repo.upsert(&profile("persisted")).await.unwrap();
        }
        let repo2 = TomlProfileRepository::new(&path);
        assert_eq!(repo2.list().await.unwrap()[0].name, "persisted");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn file_is_created_with_0600_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let (repo, _d) = repo();
        repo.upsert(&profile("alpha")).await.unwrap();
        let mode = std::fs::metadata(&repo.path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
