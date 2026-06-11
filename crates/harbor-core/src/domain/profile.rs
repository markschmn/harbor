//! Server connection profiles — the saved entries shown in the connection
//! manager.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use super::auth::AuthMethod;
use super::error::{HarborError, Result};

/// Stable identifier for a [`ServerProfile`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProfileId(pub Uuid);

impl ProfileId {
    /// Generate a fresh random id.
    pub fn new() -> Self {
        ProfileId(Uuid::new_v4())
    }
}

impl Default for ProfileId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for ProfileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for ProfileId {
    type Err = HarborError;
    fn from_str(s: &str) -> Result<Self> {
        Uuid::parse_str(s)
            .map(ProfileId)
            .map_err(|e| HarborError::validation(format!("invalid profile id: {e}")))
    }
}

/// The default SSH port.
pub const DEFAULT_SSH_PORT: u16 = 22;

/// A saved connection profile.
///
/// Profiles are persisted as TOML. They never contain secret material — only a
/// description of *how* to connect. See [`AuthMethod`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerProfile {
    pub id: ProfileId,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(default)]
    pub auth: AuthMethod,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub favorite: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

/// A builder-ish input used when creating or updating a profile from the UI.
/// Keeps the public API ergonomic and centralises validation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProfileDraft {
    pub name: String,
    pub host: String,
    pub port: Option<u16>,
    pub username: String,
    #[serde(default)]
    pub auth: AuthMethod,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub favorite: bool,
}

impl ServerProfile {
    /// Create a validated profile from a draft, stamping timestamps.
    pub fn from_draft(draft: ProfileDraft, now: OffsetDateTime) -> Result<Self> {
        let profile = ServerProfile {
            id: ProfileId::new(),
            name: draft.name,
            host: draft.host,
            port: draft.port.unwrap_or(DEFAULT_SSH_PORT),
            username: draft.username,
            auth: draft.auth,
            notes: draft.notes,
            tags: normalise_tags(draft.tags),
            favorite: draft.favorite,
            created_at: now,
            updated_at: now,
        };
        profile.validate()?;
        Ok(profile)
    }

    /// Apply an edit draft onto an existing profile, preserving id and
    /// creation time, and re-validating.
    pub fn apply_draft(&mut self, draft: ProfileDraft, now: OffsetDateTime) -> Result<()> {
        self.name = draft.name;
        self.host = draft.host;
        self.port = draft.port.unwrap_or(DEFAULT_SSH_PORT);
        self.username = draft.username;
        self.auth = draft.auth;
        self.notes = draft.notes;
        self.tags = normalise_tags(draft.tags);
        self.favorite = draft.favorite;
        self.updated_at = now;
        self.validate()
    }

    /// Enforce domain invariants.
    pub fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            return Err(HarborError::validation("profile name must not be empty"));
        }
        if self.host.trim().is_empty() {
            return Err(HarborError::validation("host must not be empty"));
        }
        if self.host.contains(char::is_whitespace) {
            return Err(HarborError::validation("host must not contain whitespace"));
        }
        if self.port == 0 {
            return Err(HarborError::validation("port must be between 1 and 65535"));
        }
        if self.username.trim().is_empty() {
            return Err(HarborError::validation("username must not be empty"));
        }
        Ok(())
    }

    /// `host:port`, the canonical address used for known_hosts lookups.
    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// Lower-cased haystack used by the search feature.
    pub fn matches_query(&self, query: &str) -> bool {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return true;
        }
        self.name.to_lowercase().contains(&q)
            || self.host.to_lowercase().contains(&q)
            || self.username.to_lowercase().contains(&q)
            || self.tags.iter().any(|t| t.to_lowercase().contains(&q))
    }
}

fn normalise_tags(tags: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    tags.into_iter()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .filter(|t| seen.insert(t.to_lowercase()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft() -> ProfileDraft {
        ProfileDraft {
            name: "Prod web".into(),
            host: "web.example.com".into(),
            port: Some(2222),
            username: "deploy".into(),
            tags: vec!["prod".into(), "  web ".into(), "PROD".into()],
            ..Default::default()
        }
    }

    #[test]
    fn from_draft_defaults_port_and_dedupes_tags() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let mut d = draft();
        d.port = None;
        let p = ServerProfile::from_draft(d, now).unwrap();
        assert_eq!(p.port, DEFAULT_SSH_PORT);
        // "prod", "web" — case-insensitive dedupe, trimmed
        assert_eq!(p.tags, vec!["prod".to_string(), "web".to_string()]);
    }

    #[test]
    fn validation_rejects_empty_fields() {
        let now = OffsetDateTime::UNIX_EPOCH;
        for mutate in [
            |d: &mut ProfileDraft| d.name = "  ".into(),
            |d: &mut ProfileDraft| d.host = "".into(),
            |d: &mut ProfileDraft| d.username = "".into(),
        ] {
            let mut d = draft();
            mutate(&mut d);
            assert!(ServerProfile::from_draft(d, now).is_err());
        }
    }

    #[test]
    fn rejects_host_with_whitespace() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let mut d = draft();
        d.host = "bad host".into();
        assert!(ServerProfile::from_draft(d, now).is_err());
    }

    #[test]
    fn search_matches_name_host_and_tags() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let p = ServerProfile::from_draft(draft(), now).unwrap();
        assert!(p.matches_query("prod"));
        assert!(p.matches_query("WEB.example"));
        assert!(p.matches_query("deploy"));
        assert!(p.matches_query("")); // empty query matches everything
        assert!(!p.matches_query("staging"));
    }

    #[test]
    fn profile_round_trips_through_toml() {
        let now = OffsetDateTime::UNIX_EPOCH;
        let p = ServerProfile::from_draft(draft(), now).unwrap();
        let s = toml::to_string(&p).unwrap();
        let parsed: ServerProfile = toml::from_str(&s).unwrap();
        assert_eq!(p, parsed);
    }
}
