# Security model

Security is a primary goal of Harbor. This document describes the security
model, the guarantees Harbor makes, and the threats it does and does not defend
against.

## Principles

1. **Never roll our own crypto.** All cryptographic primitives come from
   established, audited libraries (`russh`, RustCrypto's `ssh-key`, `sha1`/`hmac`
   for `known_hosts` hashing only). Harbor implements *policy*, not primitives.
2. **Verify host identity, always.** Host-key verification is never disabled.
   There is no "ignore host key" toggle.
3. **No plaintext secrets.** Passwords and passphrases are never written to disk
   in plaintext; they live in the OS keychain and, in memory, in zero-on-drop
   wrappers.
4. **Fail closed.** When a security decision is ambiguous or a prompt cannot be
   delivered, Harbor refuses the connection rather than proceeding.
5. **Least surprise / OpenSSH compatibility.** Harbor follows OpenSSH
   conventions (`known_hosts`, `~/.ssh` layout, key formats) so it interoperates
   with the tools administrators already trust.

## Host-key verification

Harbor identifies servers by their public host key, following the OpenSSH
Trust-On-First-Use (TOFU) model. The trust decision is made in pure, unit-tested
code (`infrastructure::known_hosts::evaluate_entries`) **before** any
authentication is attempted.

| Situation | Decision | Behaviour |
|-----------|----------|-----------|
| Presented key matches a trusted `known_hosts` entry | **Trusted** | Connect silently. |
| Host never seen before | **Unknown** | Prompt the user with the SHA‑256 fingerprint; on "Trust & save" the key is appended to `~/.ssh/known_hosts`. |
| Host known, but the key is **different** | **Mismatch** | Hard failure — connection refused. This is the "REMOTE HOST IDENTIFICATION HAS CHANGED" case and may indicate a man-in-the-middle attack. |
| Key marked `@revoked` | **Revoked** | Hard failure — always refused. |

Notes:

- The `known_hosts` parser handles plain hostnames, comma-separated patterns,
  wildcards (`*`/`?`), bracketed non-default ports (`[host]:2222`), `@revoked`
  / `@cert-authority` markers, and **hashed** (`|1|salt|hash`) entries
  (HMAC‑SHA1, matching OpenSSH).
- A *changed key for a known host* is treated as a hard failure even when only
  the key **type** is new, because a key Harbor did not record is
  indistinguishable from an attacker's key. Users can explicitly "forget" a host
  to re-establish trust.
- Harbor reads and writes the standard `~/.ssh/known_hosts`, so trust decisions
  are shared with the system `ssh` client.

## Authentication

Harbor supports the standard SSH authentication methods:

- **SSH agent** (`ssh-agent` / Pageant) — signing is delegated to the agent;
  no key material enters Harbor's process.
- **Public key** — OpenSSH-format private keys on disk.
- **Encrypted private keys** — decrypted transiently at connect time with a
  passphrase supplied by the user (or fetched from the keychain); the decrypted
  key never leaves memory.
- **Password** — supplied interactively or fetched from the keychain.

For RSA keys, Harbor requests modern `rsa-sha2-256` signatures rather than the
legacy SHA‑1 `ssh-rsa`.

## Secret storage

- **Profiles never contain secrets.** A `ServerProfile` stores only the
  `AuthMethod` (agent / password / key-path) — a description of *how* to
  authenticate, safe to serialise to `profiles.toml`.
- **Keychain-backed.** Passwords (keyed by profile id) and key passphrases
  (keyed by key path) are stored in the OS keychain via the `keyring` crate:
  macOS Keychain, Windows Credential Manager, or the Linux Secret Service.
- **Safe fallback.** Where no keychain is reachable (e.g. a headless CI box),
  Harbor falls back to a **session-only, in-memory** store and tells the user.
  The fallback still **never writes secrets to disk** — they simply do not
  persist across restarts.
- **Zero-on-drop in memory.** Secrets in memory are wrapped in
  `secrecy::SecretString`, which zeroes the buffer on drop and refuses to print
  itself in `Debug`/logs.
- **0600 config files.** `profiles.toml` is written atomically and, on Unix,
  with `0600` permissions; `known_hosts` is written `0600` inside a `0700`
  `~/.ssh`.

## Cryptography

| Concern | Library | Notes |
|---------|---------|-------|
| SSH transport, KEX, ciphers, MAC | `russh` | Modern algorithm set (Ed25519, ECDSA, ChaCha20-Poly1305, AES-GCM, …). |
| Key parsing, fingerprints, encrypted-key decryption | `ssh-key` (RustCrypto) | SHA‑256 fingerprints in OpenSSH `SHA256:` format. |
| `known_hosts` hashed-host matching | `hmac` + `sha1` | HMAC‑SHA1 only, to match the OpenSSH hashing scheme. |

Harbor contains **no hand-written cryptographic primitives**. The Rust core sets
`#![forbid(unsafe_code)]`.

## Frontend hardening

- A restrictive Content-Security-Policy is configured in `tauri.conf.json`
  (`script-src 'self'`, no remote origins).
- The webview's capability set grants only the plugin permissions the UI needs
  (dialogs, OS info); it does **not** expose arbitrary shell or filesystem
  access. All privileged operations go through explicit, typed Tauri commands.
- Terminal output is transported as base64 and written to xterm.js, which parses
  control sequences in a sandboxed renderer.

## Threat considerations

**Harbor aims to protect against:**

- Man-in-the-middle attacks via host-key substitution (verification is
  mandatory and a changed key aborts the connection).
- Accidental disclosure of passwords/passphrases through logs, crash dumps, or
  config files (zero-on-drop wrappers; keychain storage; no plaintext).
- Connecting to the wrong host by surfacing the fingerprint for out-of-band
  verification on first use.

**Harbor does not protect against (out of scope):**

- A compromised local machine. If an attacker controls the OS, the SSH agent, or
  the keychain, they can already act as the user. Harbor trusts the local
  platform's security boundaries.
- Weaknesses in the OS keychain itself.
- A user who deliberately accepts an unknown/changed host key after being shown
  the warning.
- Side-channel attacks against the underlying crypto libraries.
- Supply-chain compromise of dependencies (mitigated only by pinning and
  `Cargo.lock`).

## Reporting a vulnerability

This is a reference/educational project. For a production deployment, please
add a `SECURITY` contact and a coordinated-disclosure process here. Do not file
security issues in a public tracker without prior coordination.
