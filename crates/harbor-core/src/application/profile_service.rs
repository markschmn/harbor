//! Connection-manager use cases: CRUD, search, favourites and the
//! profile-scoped password secret.

use std::sync::Arc;

use secrecy::SecretString;
use time::OffsetDateTime;

use crate::domain::error::{HarborError, Result};
use crate::domain::profile::{ProfileDraft, ProfileId, ServerProfile};

use super::ports::{ProfileRepository, SecretRef, SecretStore};

/// Orchestrates everything the connection manager needs.
pub struct ProfileService {
    repo: Arc<dyn ProfileRepository>,
    secrets: Arc<dyn SecretStore>,
}

impl std::fmt::Debug for ProfileService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ProfileService")
    }
}

impl ProfileService {
    pub fn new(repo: Arc<dyn ProfileRepository>, secrets: Arc<dyn SecretStore>) -> Self {
        Self { repo, secrets }
    }

    /// All profiles, sorted favourites-first then by name.
    pub async fn list(&self) -> Result<Vec<ServerProfile>> {
        let mut profiles = self.repo.list().await?;
        profiles.sort_by(|a, b| {
            b.favorite
                .cmp(&a.favorite)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(profiles)
    }

    /// Profiles matching a free-text query (name/host/username/tags).
    pub async fn search(&self, query: &str) -> Result<Vec<ServerProfile>> {
        let all = self.list().await?;
        Ok(all.into_iter().filter(|p| p.matches_query(query)).collect())
    }

    pub async fn get(&self, id: ProfileId) -> Result<ServerProfile> {
        self.repo.get(id).await
    }

    /// Create and persist a new profile from a UI draft.
    pub async fn create(&self, draft: ProfileDraft) -> Result<ServerProfile> {
        let profile = ServerProfile::from_draft(draft, OffsetDateTime::now_utc())?;
        self.repo.upsert(&profile).await?;
        Ok(profile)
    }

    /// Apply an edit to an existing profile.
    pub async fn update(&self, id: ProfileId, draft: ProfileDraft) -> Result<ServerProfile> {
        let mut profile = self.repo.get(id).await?;
        profile.apply_draft(draft, OffsetDateTime::now_utc())?;
        self.repo.upsert(&profile).await?;
        Ok(profile)
    }

    /// Delete a profile and any secret associated with it.
    pub async fn delete(&self, id: ProfileId) -> Result<()> {
        // Best-effort secret cleanup; a missing secret is not an error.
        let _ = self.secrets.delete(&SecretRef::ProfilePassword(id)).await;
        self.repo.delete(id).await
    }

    /// Flip the favourite flag and persist.
    pub async fn toggle_favorite(&self, id: ProfileId) -> Result<ServerProfile> {
        let mut profile = self.repo.get(id).await?;
        profile.favorite = !profile.favorite;
        profile.updated_at = OffsetDateTime::now_utc();
        self.repo.upsert(&profile).await?;
        Ok(profile)
    }

    /// Store (or replace) the login password for a profile in the keychain.
    pub async fn set_password(&self, id: ProfileId, password: SecretString) -> Result<()> {
        // Ensure the profile exists before writing a secret for it.
        self.repo.get(id).await?;
        self.secrets
            .set(&SecretRef::ProfilePassword(id), password)
            .await
    }

    /// Retrieve a stored password, if any.
    pub async fn get_password(&self, id: ProfileId) -> Result<Option<SecretString>> {
        self.secrets.get(&SecretRef::ProfilePassword(id)).await
    }

    /// Whether a password is stored for a profile (without revealing it).
    pub async fn has_password(&self, id: ProfileId) -> Result<bool> {
        Ok(self.get_password(id).await?.is_some())
    }

    /// Delete only the stored password, keeping the profile.
    pub async fn clear_password(&self, id: ProfileId) -> Result<()> {
        self.secrets.delete(&SecretRef::ProfilePassword(id)).await
    }

    /// Look up a profile by id, returning a friendly error if missing.
    pub async fn require(&self, id: ProfileId) -> Result<ServerProfile> {
        self.repo
            .get(id)
            .await
            .map_err(|_| HarborError::NotFound(format!("profile {id}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{InMemoryProfileRepo, InMemorySecretStore};

    fn service() -> ProfileService {
        ProfileService::new(
            Arc::new(InMemoryProfileRepo::default()),
            Arc::new(InMemorySecretStore::default()),
        )
    }

    fn draft(name: &str) -> ProfileDraft {
        ProfileDraft {
            name: name.into(),
            host: "example.com".into(),
            username: "me".into(),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn create_list_update_delete() {
        let svc = service();
        let created = svc.create(draft("alpha")).await.unwrap();
        assert_eq!(svc.list().await.unwrap().len(), 1);

        let mut d = draft("alpha-renamed");
        d.favorite = true;
        let updated = svc.update(created.id, d).await.unwrap();
        assert_eq!(updated.name, "alpha-renamed");
        assert!(updated.favorite);
        assert_eq!(updated.id, created.id, "id is stable across updates");

        svc.delete(created.id).await.unwrap();
        assert!(svc.list().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn list_sorts_favorites_first_then_by_name() {
        let svc = service();
        svc.create(draft("zeta")).await.unwrap();
        let beta = svc.create(draft("beta")).await.unwrap();
        svc.create(draft("alpha")).await.unwrap();
        svc.toggle_favorite(beta.id).await.unwrap();

        let names: Vec<_> = svc
            .list()
            .await
            .unwrap()
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert_eq!(names, vec!["beta", "alpha", "zeta"]);
    }

    #[tokio::test]
    async fn password_is_stored_and_cleared_via_secret_store() {
        let svc = service();
        let p = svc.create(draft("alpha")).await.unwrap();
        assert!(!svc.has_password(p.id).await.unwrap());

        svc.set_password(p.id, SecretString::from("s3cret"))
            .await
            .unwrap();
        assert!(svc.has_password(p.id).await.unwrap());
        use secrecy::ExposeSecret;
        assert_eq!(
            svc.get_password(p.id)
                .await
                .unwrap()
                .unwrap()
                .expose_secret(),
            "s3cret"
        );

        svc.clear_password(p.id).await.unwrap();
        assert!(!svc.has_password(p.id).await.unwrap());
    }

    #[tokio::test]
    async fn deleting_profile_also_removes_password() {
        let svc = service();
        let p = svc.create(draft("alpha")).await.unwrap();
        svc.set_password(p.id, SecretString::from("s3cret"))
            .await
            .unwrap();
        svc.delete(p.id).await.unwrap();
        // Re-create to reuse the store; the old secret must be gone for the id.
        assert!(svc.get_password(p.id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn search_filters_results() {
        let svc = service();
        svc.create(draft("production")).await.unwrap();
        svc.create(draft("staging")).await.unwrap();
        assert_eq!(svc.search("prod").await.unwrap().len(), 1);
        assert_eq!(svc.search("").await.unwrap().len(), 2);
    }
}
