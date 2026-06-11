//! OpenSSH `known_hosts` parsing, host-key evaluation and persistence.
//!
//! This is the heart of Harbor's transport security. The functions here are
//! pure and exhaustively unit-tested; the file-backed [`FileKnownHostsStore`]
//! is a thin I/O wrapper around them.
//!
//! ## Security policy
//!
//! Harbor follows OpenSSH conventions and errs on the side of refusing
//! connections:
//!
//! * A presented key that exactly matches a trusted entry → **Trusted**.
//! * A presented key matching a `@revoked` entry → **Revoked** (always refuse).
//! * A host we have *never* seen → **Unknown** (Trust On First Use prompt).
//! * A host we *have* seen, presenting a key we do **not** have on record →
//!   **Mismatch**. This is the "REMOTE HOST IDENTIFICATION HAS CHANGED" case and
//!   is treated as a hard failure even if only the key *type* is new, because a
//!   key we did not record is indistinguishable from an attacker's key.

use std::path::PathBuf;
use std::sync::Mutex;

use async_trait::async_trait;
use base64::Engine;
use hmac::{Hmac, Mac};
use sha1::Sha1;

use crate::application::ports::KnownHostsStore;
use crate::domain::error::{HarborError, Result};
use crate::domain::host_key::{HostKey, HostKeyDecision, KnownHostEntry};

use super::paths;

const REVOKED_MARKER: &str = "@revoked";

/// Parse the textual contents of a `known_hosts` file into entries.
///
/// Malformed lines are skipped (matching OpenSSH leniency) rather than failing
/// the whole file.
pub fn parse_known_hosts(text: &str) -> Vec<KnownHostEntry> {
    let mut entries = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut tokens = line.split_whitespace();
        let Some(mut first) = tokens.next() else {
            continue;
        };

        let mut marker = None;
        if first.starts_with('@') {
            marker = Some(first.to_string());
            match tokens.next() {
                Some(t) => first = t,
                None => continue,
            }
        }

        let host_field = first.to_string();
        let Some(algorithm) = tokens.next() else {
            continue;
        };
        let Some(key_base64) = tokens.next() else {
            continue;
        };

        let hashed = host_field.starts_with("|1|");
        entries.push(KnownHostEntry {
            host_field,
            key: HostKey::new(algorithm, key_base64),
            hashed,
            marker,
        });
    }
    entries
}

/// The canonical host token used both for hashing and for `[host]:port` style
/// patterns. OpenSSH omits the brackets for the default port.
pub fn canonical_host(host: &str, port: u16) -> String {
    if port == 22 {
        host.to_string()
    } else {
        format!("[{host}]:{port}")
    }
}

/// The host field Harbor writes for a newly trusted key.
pub fn host_pattern(host: &str, port: u16) -> String {
    canonical_host(host, port)
}

/// Evaluate a presented host key against a set of `known_hosts` entries.
///
/// This is the single source of truth for the trust decision; both the
/// file-backed and in-memory stores delegate to it.
pub fn evaluate_entries(
    entries: &[KnownHostEntry],
    host: &str,
    port: u16,
    presented: &HostKey,
) -> HostKeyDecision {
    let fingerprint = fingerprint(presented).unwrap_or_else(|_| "SHA256:<unknown>".into());

    let matching: Vec<&KnownHostEntry> = entries
        .iter()
        .filter(|e| host_field_matches(&e.host_field, e.hashed, host, port))
        .collect();

    if matching.is_empty() {
        return HostKeyDecision::Unknown { fingerprint };
    }

    // Exact key match wins (respecting an explicit revocation).
    for e in &matching {
        if keys_equal(&e.key, presented) {
            if e.marker.as_deref() == Some(REVOKED_MARKER) {
                return HostKeyDecision::Revoked { fingerprint };
            }
            return HostKeyDecision::Trusted;
        }
    }

    // Host known, but the presented key is not on record: hard failure.
    let mut expected_algorithms: Vec<String> = matching
        .iter()
        .filter(|e| e.marker.is_none())
        .map(|e| e.key.algorithm.clone())
        .collect();
    expected_algorithms.sort();
    expected_algorithms.dedup();
    HostKeyDecision::Mismatch {
        fingerprint,
        expected_algorithms,
    }
}

/// SHA256 fingerprint in OpenSSH format (`SHA256:base64`) of a host key.
pub fn fingerprint(key: &HostKey) -> Result<String> {
    let openssh = format!("{} {}", key.algorithm, key.key_base64);
    let public = ssh_key::PublicKey::from_openssh(&openssh)
        .map_err(|e| HarborError::Key(format!("invalid host key: {e}")))?;
    Ok(public
        .fingerprint(ssh_key::HashAlg::Sha256)
        .to_string())
}

/// Compare two host keys by their decoded wire bytes (robust to base64
/// formatting differences).
fn keys_equal(a: &HostKey, b: &HostKey) -> bool {
    match (decode_b64(&a.key_base64), decode_b64(&b.key_base64)) {
        (Some(x), Some(y)) => x == y,
        // Fall back to a case-sensitive textual comparison if either fails.
        _ => a.key_base64 == b.key_base64,
    }
}

fn decode_b64(s: &str) -> Option<Vec<u8>> {
    base64::engine::general_purpose::STANDARD.decode(s).ok()
}

/// Whether a (possibly hashed, possibly comma-separated, possibly wildcarded)
/// host field matches `host:port`.
fn host_field_matches(field: &str, hashed: bool, host: &str, port: u16) -> bool {
    if hashed {
        return hashed_matches(field, &canonical_host(host, port));
    }

    // OpenSSH allows comma-separated patterns and negations (`!pattern`).
    let mut matched = false;
    for pattern in field.split(',') {
        let pattern = pattern.trim();
        if let Some(neg) = pattern.strip_prefix('!') {
            if pattern_matches(neg, host, port) {
                return false; // an explicit negation excludes the host
            }
        } else if pattern_matches(pattern, host, port) {
            matched = true;
        }
    }
    matched
}

/// Match a single (non-hashed) pattern against `host:port`.
fn pattern_matches(pattern: &str, host: &str, port: u16) -> bool {
    if let Some(rest) = pattern.strip_prefix('[') {
        // Bracketed `[host]:port` form, used for non-default ports.
        if let Some((host_part, port_part)) = rest.split_once("]:") {
            let port_ok = port_part.parse::<u16>().map(|p| p == port).unwrap_or(false);
            return port_ok && glob_match(host_part, host);
        }
        return false;
    }
    // A bare host pattern only applies to the default SSH port.
    port == 22 && glob_match(pattern, host)
}

/// Match a hashed `|1|salt|hash` field against a host token using HMAC-SHA1.
fn hashed_matches(field: &str, host_token: &str) -> bool {
    // Format: |1|<base64 salt>|<base64 hash>
    let mut parts = field.split('|');
    // Leading empty (before first '|'), then "1", salt, hash.
    if parts.next() != Some("") {
        return false;
    }
    if parts.next() != Some("1") {
        return false;
    }
    let (Some(salt_b64), Some(hash_b64)) = (parts.next(), parts.next()) else {
        return false;
    };
    let (Some(salt), Some(expected)) = (decode_b64(salt_b64), decode_b64(hash_b64)) else {
        return false;
    };

    let Ok(mut mac) = Hmac::<Sha1>::new_from_slice(&salt) else {
        return false;
    };
    mac.update(host_token.as_bytes());
    mac.verify_slice(&expected).is_ok()
}

/// Minimal glob matcher supporting `*` (any run) and `?` (single char), which
/// is the subset OpenSSH uses in host patterns. Case-insensitive on the host.
fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.to_ascii_lowercase().chars().collect();
    let t: Vec<char> = text.to_ascii_lowercase().chars().collect();
    glob_rec(&p, &t)
}

fn glob_rec(p: &[char], t: &[char]) -> bool {
    match p.first() {
        None => t.is_empty(),
        Some('*') => {
            // Match zero or more characters.
            glob_rec(&p[1..], t) || (!t.is_empty() && glob_rec(p, &t[1..]))
        }
        Some('?') => !t.is_empty() && glob_rec(&p[1..], &t[1..]),
        Some(&c) => !t.is_empty() && t[0] == c && glob_rec(&p[1..], &t[1..]),
    }
}

/// File-backed `known_hosts` store using the standard OpenSSH location so trust
/// decisions are shared with the system `ssh` client.
#[derive(Debug)]
pub struct FileKnownHostsStore {
    path: PathBuf,
    write_lock: Mutex<()>,
}

impl FileKnownHostsStore {
    pub fn with_default_location() -> Result<Self> {
        Ok(Self::new(paths::known_hosts_path()?))
    }

    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            write_lock: Mutex::new(()),
        }
    }

    fn read_entries(&self) -> Result<Vec<KnownHostEntry>> {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => Ok(parse_known_hosts(&text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(HarborError::Io {
                path: self.path.clone(),
                source: e,
            }),
        }
    }
}

#[async_trait]
impl KnownHostsStore for FileKnownHostsStore {
    async fn evaluate(
        &self,
        host: &str,
        port: u16,
        presented: &HostKey,
    ) -> Result<HostKeyDecision> {
        let entries = self.read_entries()?;
        Ok(evaluate_entries(&entries, host, port, presented))
    }

    async fn trust(&self, host: &str, port: u16, key: &HostKey) -> Result<()> {
        use std::io::Write;
        let _guard = self.write_lock.lock().unwrap();

        if let Some(parent) = self.path.parent() {
            paths::ensure_dir(parent)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(
                    parent,
                    std::fs::Permissions::from_mode(0o700),
                );
            }
        }

        let line = format!("{} {}\n", host_pattern(host, port), key.authorized_key());
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| HarborError::Io {
                path: self.path.clone(),
                source: e,
            })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = file.set_permissions(std::fs::Permissions::from_mode(0o600));
        }
        file.write_all(line.as_bytes()).map_err(|e| HarborError::Io {
            path: self.path.clone(),
            source: e,
        })
    }

    async fn forget(&self, host: &str, port: u16) -> Result<()> {
        let _guard = self.write_lock.lock().unwrap();
        let text = match std::fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => {
                return Err(HarborError::Io {
                    path: self.path.clone(),
                    source: e,
                })
            }
        };

        let kept: String = text
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    return true; // keep comments and blanks
                }
                // Drop lines whose (single) entry matches the host.
                match parse_known_hosts(line).first() {
                    Some(entry) => {
                        !host_field_matches(&entry.host_field, entry.hashed, host, port)
                    }
                    None => true,
                }
            })
            .map(|l| format!("{l}\n"))
            .collect();

        std::fs::write(&self.path, kept).map_err(|e| HarborError::Io {
            path: self.path.clone(),
            source: e,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real ed25519 public keys (matching the checked-in fixtures), so the
    // fingerprint and byte-comparison paths are genuinely exercised.
    const ED25519_LINE: &str =
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGR8zYp9mBIFQ9wN3ER/lUJWFGPoT1AT1CxTZnG+Arzn";
    // A *different* real ed25519 key for the same host (simulated key change).
    const ED25519_OTHER: &str =
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIHR1VLZ8uFbq13WOvgPLijFCD1COlDFmkWX2Eq4fzXON";

    fn key(line: &str) -> HostKey {
        let mut it = line.split_whitespace();
        HostKey::new(it.next().unwrap(), it.next().unwrap())
    }

    #[test]
    fn parses_plain_entry() {
        let entries = parse_known_hosts(&format!("example.com {ED25519_LINE}"));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].host_field, "example.com");
        assert_eq!(entries[0].key.algorithm, "ssh-ed25519");
        assert!(!entries[0].hashed);
    }

    #[test]
    fn skips_comments_and_blank_lines() {
        let text = format!("# a comment\n\n   \nexample.com {ED25519_LINE}\n");
        assert_eq!(parse_known_hosts(&text).len(), 1);
    }

    #[test]
    fn parses_marker_and_bracketed_port() {
        let text = format!("@revoked [example.com]:2222 {ED25519_LINE}");
        let entries = parse_known_hosts(&text);
        assert_eq!(entries[0].marker.as_deref(), Some("@revoked"));
        assert_eq!(entries[0].host_field, "[example.com]:2222");
    }

    #[test]
    fn known_matching_key_is_trusted() {
        let entries = parse_known_hosts(&format!("example.com {ED25519_LINE}"));
        let decision = evaluate_entries(&entries, "example.com", 22, &key(ED25519_LINE));
        assert_eq!(decision, HostKeyDecision::Trusted);
    }

    #[test]
    fn unseen_host_is_unknown() {
        let entries = parse_known_hosts(&format!("other.com {ED25519_LINE}"));
        let decision = evaluate_entries(&entries, "example.com", 22, &key(ED25519_LINE));
        assert!(matches!(decision, HostKeyDecision::Unknown { .. }));
    }

    #[test]
    fn changed_key_for_known_host_is_mismatch() {
        let entries = parse_known_hosts(&format!("example.com {ED25519_LINE}"));
        let decision = evaluate_entries(&entries, "example.com", 22, &key(ED25519_OTHER));
        assert!(decision.is_hard_failure());
        assert!(matches!(decision, HostKeyDecision::Mismatch { .. }));
    }

    #[test]
    fn revoked_key_is_refused() {
        let entries = parse_known_hosts(&format!("@revoked example.com {ED25519_LINE}"));
        let decision = evaluate_entries(&entries, "example.com", 22, &key(ED25519_LINE));
        assert!(matches!(decision, HostKeyDecision::Revoked { .. }));
        assert!(decision.is_hard_failure());
    }

    #[test]
    fn non_default_port_requires_bracketed_entry() {
        // A bare host entry must NOT match a non-default port.
        let entries = parse_known_hosts(&format!("example.com {ED25519_LINE}"));
        let decision = evaluate_entries(&entries, "example.com", 2222, &key(ED25519_LINE));
        assert!(matches!(decision, HostKeyDecision::Unknown { .. }));

        // The bracketed form matches.
        let entries = parse_known_hosts(&format!("[example.com]:2222 {ED25519_LINE}"));
        let decision = evaluate_entries(&entries, "example.com", 2222, &key(ED25519_LINE));
        assert_eq!(decision, HostKeyDecision::Trusted);
    }

    #[test]
    fn comma_separated_and_wildcard_patterns() {
        let entries = parse_known_hosts(&format!("alias.com,*.example.com {ED25519_LINE}"));
        let decision = evaluate_entries(&entries, "web.example.com", 22, &key(ED25519_LINE));
        assert_eq!(decision, HostKeyDecision::Trusted);
    }

    #[test]
    fn glob_matching_rules() {
        assert!(glob_match("*.example.com", "a.example.com"));
        assert!(glob_match("host?", "host1"));
        assert!(!glob_match("host?", "host"));
        assert!(glob_match("*", "anything"));
        assert!(glob_match("example.com", "EXAMPLE.COM")); // case-insensitive
        assert!(!glob_match("a.example.com", "b.example.com"));
    }

    #[test]
    fn hashed_known_hosts_matching() {
        // Construct a hashed entry the same way OpenSSH does, so we exercise the
        // real HMAC-SHA1 path rather than a hand-rolled fixture.
        let salt = b"0123456789abcdef01234567"; // 24 bytes
        let host_token = "example.com"; // default port → unbracketed
        let mut mac = Hmac::<Sha1>::new_from_slice(salt).unwrap();
        mac.update(host_token.as_bytes());
        let hash = mac.finalize().into_bytes();
        let salt_b64 = base64::engine::general_purpose::STANDARD.encode(salt);
        let hash_b64 = base64::engine::general_purpose::STANDARD.encode(hash);
        let line = format!("|1|{salt_b64}|{hash_b64} {ED25519_LINE}");

        let entries = parse_known_hosts(&line);
        assert!(entries[0].hashed);
        let decision = evaluate_entries(&entries, "example.com", 22, &key(ED25519_LINE));
        assert_eq!(decision, HostKeyDecision::Trusted);

        // A different host must not match the same hash.
        let decision = evaluate_entries(&entries, "evil.com", 22, &key(ED25519_LINE));
        assert!(matches!(decision, HostKeyDecision::Unknown { .. }));
    }

    #[tokio::test]
    async fn file_store_tofu_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = FileKnownHostsStore::new(dir.path().join("known_hosts"));
        let k = key(ED25519_LINE);

        // Initially unknown.
        let d = store.evaluate("example.com", 22, &k).await.unwrap();
        assert!(matches!(d, HostKeyDecision::Unknown { .. }));

        // Trust, then it is trusted on the next evaluation.
        store.trust("example.com", 22, &k).await.unwrap();
        let d = store.evaluate("example.com", 22, &k).await.unwrap();
        assert_eq!(d, HostKeyDecision::Trusted);

        // A changed key is now a mismatch.
        let d = store
            .evaluate("example.com", 22, &key(ED25519_OTHER))
            .await
            .unwrap();
        assert!(d.is_hard_failure());

        // Forget removes the host entirely.
        store.forget("example.com", 22).await.unwrap();
        let d = store.evaluate("example.com", 22, &k).await.unwrap();
        assert!(matches!(d, HostKeyDecision::Unknown { .. }));
    }
}
