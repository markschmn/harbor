//! Non-interactive [`HostKeyPrompter`] implementations.
//!
//! These resolve an *unknown* host (Trust On First Use) without a user
//! interface. The desktop app supplies its own interactive prompter that asks
//! the user; these are for headless callers, tests, and explicit policies.
//!
//! Note: a prompter is only ever consulted for the **Unknown** case. A
//! `Mismatch` or `Revoked` decision is a hard failure decided by
//! [`known_hosts`](super::known_hosts) and is never delegated here.

use async_trait::async_trait;

use crate::application::ports::{HostKeyPrompter, HostKeyPrompt};
use crate::domain::host_key::TofuResolution;

/// Accept and persist unknown host keys (equivalent to OpenSSH
/// `StrictHostKeyChecking=accept-new`). This is a safe default: it never
/// accepts a *changed* key, only brand-new hosts.
#[derive(Debug, Default, Clone, Copy)]
pub struct AcceptNewPolicy;

#[async_trait]
impl HostKeyPrompter for AcceptNewPolicy {
    async fn resolve_unknown(&self, _prompt: HostKeyPrompt) -> TofuResolution {
        TofuResolution::TrustAndSave
    }
}

/// Refuse unknown hosts (equivalent to `StrictHostKeyChecking=yes`). The most
/// conservative policy; suitable for automation against pre-provisioned hosts.
#[derive(Debug, Default, Clone, Copy)]
pub struct StrictPolicy;

#[async_trait]
impl HostKeyPrompter for StrictPolicy {
    async fn resolve_unknown(&self, _prompt: HostKeyPrompt) -> TofuResolution {
        TofuResolution::Reject
    }
}

/// Trust unknown hosts for the current session only, without persisting.
#[derive(Debug, Default, Clone, Copy)]
pub struct TrustOncePolicy;

#[async_trait]
impl HostKeyPrompter for TrustOncePolicy {
    async fn resolve_unknown(&self, _prompt: HostKeyPrompt) -> TofuResolution {
        TofuResolution::TrustOnce
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prompt() -> HostKeyPrompt {
        HostKeyPrompt {
            host: "h".into(),
            port: 22,
            algorithm: "ssh-ed25519".into(),
            fingerprint: "SHA256:x".into(),
        }
    }

    #[tokio::test]
    async fn policies_resolve_as_documented() {
        assert_eq!(
            AcceptNewPolicy.resolve_unknown(prompt()).await,
            TofuResolution::TrustAndSave
        );
        assert_eq!(
            StrictPolicy.resolve_unknown(prompt()).await,
            TofuResolution::Reject
        );
        assert_eq!(
            TrustOncePolicy.resolve_unknown(prompt()).await,
            TofuResolution::TrustOnce
        );
    }
}
