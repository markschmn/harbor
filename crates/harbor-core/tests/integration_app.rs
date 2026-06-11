//! Application-level integration tests that wire the real infrastructure
//! adapters (TOML storage, file-backed known_hosts) behind the services.

use std::sync::Arc;

use harbor_core::application::ports::KnownHostsStore;
use harbor_core::application::ProfileService;
use harbor_core::domain::auth::AuthMethod;
use harbor_core::domain::host_key::{HostKey, HostKeyDecision};
use harbor_core::domain::profile::ProfileDraft;
use harbor_core::infrastructure::known_hosts::FileKnownHostsStore;
use harbor_core::infrastructure::storage::TomlProfileRepository;
use harbor_core::testing::InMemorySecretStore;
use secrecy::SecretString;

fn draft(name: &str, host: &str) -> ProfileDraft {
    ProfileDraft {
        name: name.into(),
        host: host.into(),
        username: "deploy".into(),
        auth: AuthMethod::Agent,
        ..Default::default()
    }
}

/// Profiles created through the service persist to TOML and reload identically
/// in a fresh service instance — the "configuration loading" requirement.
#[tokio::test]
async fn profiles_persist_and_reload_from_toml() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("profiles.toml");

    let prod_id = {
        let svc = ProfileService::new(
            Arc::new(TomlProfileRepository::new(&path)),
            Arc::new(InMemorySecretStore::default()),
        );
        let prod = svc.create(draft("Prod", "prod.example.com")).await.unwrap();
        svc.create(draft("Staging", "staging.example.com"))
            .await
            .unwrap();
        svc.toggle_favorite(prod.id).await.unwrap();
        prod.id
    };

    // A brand-new service over the same file sees the persisted data.
    let svc = ProfileService::new(
        Arc::new(TomlProfileRepository::new(&path)),
        Arc::new(InMemorySecretStore::default()),
    );
    let all = svc.list().await.unwrap();
    assert_eq!(all.len(), 2);
    // Favourite sorts first.
    assert_eq!(all[0].id, prod_id);
    assert!(all[0].favorite);

    // Search works end-to-end.
    assert_eq!(svc.search("staging").await.unwrap().len(), 1);

    // The TOML on disk must not be empty and must be valid.
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("prod.example.com"));
}

/// A stored password is round-tripped through the secret store and removed when
/// the profile is deleted.
#[tokio::test]
async fn profile_password_lifecycle() {
    use secrecy::ExposeSecret;
    let dir = tempfile::tempdir().unwrap();
    let svc = ProfileService::new(
        Arc::new(TomlProfileRepository::new(dir.path().join("p.toml"))),
        Arc::new(InMemorySecretStore::default()),
    );

    let p = svc.create(draft("pw", "h")).await.unwrap();
    svc.set_password(p.id, SecretString::from("s3cret"))
        .await
        .unwrap();
    assert_eq!(
        svc.get_password(p.id).await.unwrap().unwrap().expose_secret(),
        "s3cret"
    );

    svc.delete(p.id).await.unwrap();
    assert!(svc.get_password(p.id).await.unwrap().is_none());
}

/// Security: the file-backed known_hosts store enforces the OpenSSH trust model
/// through the `KnownHostsStore` port — TOFU, then mismatch is a hard failure.
#[tokio::test]
async fn known_hosts_store_enforces_trust_model() {
    let dir = tempfile::tempdir().unwrap();
    let store = FileKnownHostsStore::new(dir.path().join("known_hosts"));

    let key = HostKey::new(
        "ssh-ed25519",
        "AAAAC3NzaC1lZDI1NTE5AAAAIGR8zYp9mBIFQ9wN3ER/lUJWFGPoT1AT1CxTZnG+Arzn",
    );
    let changed = HostKey::new(
        "ssh-ed25519",
        "AAAAC3NzaC1lZDI1NTE5AAAAIHR1VLZ8uFbq13WOvgPLijFCD1COlDFmkWX2Eq4fzXON",
    );

    // First contact: unknown.
    assert!(matches!(
        store.evaluate("h.example.com", 22, &key).await.unwrap(),
        HostKeyDecision::Unknown { .. }
    ));

    // Trust on first use, then trusted.
    store.trust("h.example.com", 22, &key).await.unwrap();
    assert_eq!(
        store.evaluate("h.example.com", 22, &key).await.unwrap(),
        HostKeyDecision::Trusted
    );

    // A changed key is a hard failure (potential MITM).
    let decision = store.evaluate("h.example.com", 22, &changed).await.unwrap();
    assert!(decision.is_hard_failure());
    assert!(matches!(decision, HostKeyDecision::Mismatch { .. }));

    // The trusted key was actually written to the file in OpenSSH format.
    let text = std::fs::read_to_string(dir.path().join("known_hosts")).unwrap();
    assert!(text.contains("ssh-ed25519"));
}
