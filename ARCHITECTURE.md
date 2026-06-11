# Architecture

Harbor is built as a layered ("clean architecture") system with a strict
dependency direction: outer layers depend on inner layers, never the reverse.
This keeps the security-critical logic isolated, GUI-agnostic, and exhaustively
testable without a running desktop shell.

```
┌──────────────────────────────────────────────────────────────┐
│  Presentation                                                  │
│  React UI (src/)  ──IPC──▶  Tauri commands (src-tauri/)        │
└───────────────────────────────┬──────────────────────────────┘
                                 │ depends on
┌───────────────────────────────▼──────────────────────────────┐
│  Application  (crates/harbor-core/src/application)             │
│  ProfileService · SessionService · TransferService · KeyService│
│  + ports (traits): ProfileRepository, SecretStore,            │
│    KnownHostsStore, KeyDiscovery, SshTransport, SftpClient…    │
└───────────────────────────────┬──────────────────────────────┘
        depends on               │ implemented by
┌───────────────────────────────▼──────────────────────────────┐
│  Infrastructure (crates/harbor-core/src/infrastructure)       │
│  russh transport · russh-sftp · TOML store · keyring ·        │
│  known_hosts · ssh-key discovery                              │
└───────────────────────────────┬──────────────────────────────┘
                                 │ depends on
┌───────────────────────────────▼──────────────────────────────┐
│  Domain  (crates/harbor-core/src/domain)                      │
│  Pure entities/value objects: ServerProfile, AuthMethod,      │
│  HostKey + decision model, TransferTask, DirEntry … (no I/O)  │
└──────────────────────────────────────────────────────────────┘
```

## Layer responsibilities

| Layer | Crate / dir | Responsibility | Depends on |
|-------|-------------|----------------|-----------|
| **Domain** | `harbor-core/src/domain` | Entities, value objects, invariants. Pure, deterministic, no I/O. | — |
| **Application** | `harbor-core/src/application` | Use-case orchestration + *ports* (traits) the outer layers satisfy. | Domain |
| **Infrastructure** | `harbor-core/src/infrastructure` | Concrete adapters: SSH/SFTP, storage, keychain, `known_hosts`, key discovery. | Domain, Application (ports) |
| **Presentation** | `src-tauri` + `src` | Tauri commands, app state, event bridging; React UI. | harbor-core |

> **A note on the directory mapping.** The bootstrap brief listed
> `src/domain`, `src/application`, `src/infrastructure`, `src/presentation`.
> Tauri reserves the repository-root `src/` for the web frontend and `src-tauri/`
> for the desktop binary, so Harbor maps the four layers as: domain /
> application / infrastructure → the `harbor-core` library crate, and
> presentation → the `src-tauri` crate. This honours the layering while staying
> idiomatic for a Tauri + Vite project.

### Why a separate `harbor-core` crate?

All security-critical and business logic lives in `harbor-core`, a plain Rust
library with **no** dependency on Tauri or any GUI toolkit. Benefits:

- It compiles and is **fully unit/integration tested without a webview** — the
  parts that matter most (host-key verification, `known_hosts` parsing, transfer
  queue, config storage) are verified in milliseconds in CI.
- The GUI is a thin adapter. The same core could power a CLI or a TUI.
- The dependency arrow points inward; the UI cannot smuggle business rules in.

## Key technical decisions

### SSH library: **russh**

Harbor uses [`russh`](https://crates.io/crates/russh) (+ `russh-sftp`) for all
SSH and SFTP. The alternatives considered:

| Option | Notes | Verdict |
|--------|-------|---------|
| **russh** | Pure-Rust, async (Tokio), memory-safe, no system deps, modern algorithm set, precise host-key verification hook. | ✅ **Chosen** |
| `ssh2` (libssh2) | C library via FFI; mostly synchronous; needs OpenSSL; extra `unsafe` surface; awkward on Windows. | ✗ |
| `openssh` | Wraps the system `ssh` binary; requires OpenSSH to be installed; weak on Windows; hard to drive a PTY/SFTP programmatically. | ✗ |

Decisive factors for a **Windows-first, cross-platform GUI**:

1. **No native/C build dependency** — `russh` is pure Rust, so there is no
   libssh2/OpenSSL/`ssh.exe` to ship or detect. This matters most on Windows.
2. **Async-native** — integrates directly with the Tokio runtime that already
   drives the app, giving non-blocking terminals and concurrent transfers.
3. **Memory safety** — no `unsafe` FFI on the security-critical path.
4. **Verification control** — russh's `client::Handler::check_server_key` hook
   lets Harbor enforce its `known_hosts` policy *before* authentication, which is
   exactly where the trust decision belongs.

The trade-off — russh implements a deliberately modern subset of SSH algorithms
rather than every legacy cipher — is acceptable (and arguably desirable) for a
new client in 2026.

### File transfer: **SFTP, not SCP**

Harbor uses **SFTP** exclusively. Reasoning:

- **Real filesystem semantics.** SFTP is a stateful subsystem with directory
  listings, `stat`/`rename`/`mkdir`/`remove` and random-access reads/writes —
  everything a dual-pane file manager needs. SCP is essentially a remote `cp`
  over a shell pipe and **cannot even list a directory**.
- **Security.** The SCP protocol has a history of parsing/quoting
  vulnerabilities (e.g. CVE-2019-6111, where a malicious server could write
  files outside the requested path). OpenSSH itself now defaults `scp` to use
  the SFTP protocol under the hood and recommends SFTP.
- **Progress & resumability.** SFTP's ranged reads/writes give accurate
  progress reporting and a path to resumable transfers.

The brief asked to "prefer SFTP over SCP unless there is a compelling technical
reason not to." There is no such reason for a GUI file manager, so SCP is not
implemented.

### Secrets: OS keychain, never plaintext

Profiles store only *how* to authenticate (`AuthMethod`), never the secret.
Passwords and key passphrases live in the OS keychain (macOS Keychain, Windows
Credential Manager, Linux Secret Service) via the `keyring` crate. Secrets in
memory are wrapped in [`secrecy::SecretString`] so they are zeroed on drop and
never logged or serialised. See [SECURITY.md](SECURITY.md).

### Configuration: TOML + Serde

Connection profiles are persisted as a single `profiles.toml` in the platform
config directory (`~/.config/harbor`, `%APPDATA%\harbor`, etc.). Writes are
atomic (temp file + rename) and restricted to `0600` on Unix. The file never
contains secret material.

## Runtime flows

### Connecting + host-key verification

```
UI "Connect"
   └▶ command: connect(profile, optional secret)
        └▶ SessionService.connect(params, known_hosts, prompter)
             └▶ russh handshake
                  └▶ Handler::check_server_key(presented_key)
                       └▶ KnownHostsStore.evaluate(host, port, key)
                            ├─ Trusted          → proceed
                            ├─ Mismatch/Revoked → abort (typed HostKeyMismatch)
                            └─ Unknown          → HostKeyPrompter.resolve_unknown
                                                   (emits UI event, awaits answer)
                                                   ├─ Trust & save → write known_hosts, proceed
                                                   ├─ Trust once   → proceed
                                                   └─ Reject       → abort
             └▶ authenticate (agent / password / key / encrypted key)
```

The unknown-host prompt is bridged to the UI by `EventPrompter`: it emits an
event, parks on a `oneshot` channel, and is resumed by the `respond_host_key`
command — so the human decision happens mid-handshake without blocking the
runtime.

### Terminal I/O

`open_shell` opens a russh channel with a PTY and spawns a pump task that
`select!`s between an input channel (`ShellInput`) and `channel.wait()`. Output
bytes are base64-encoded and emitted as `harbor://terminal-data`; xterm.js
writes them. Keystrokes flow back through `send_input`; resizes through
`resize_terminal` → `window_change`.

### Transfers

`TransferService` is a concurrent queue (a Tokio `Semaphore` bounds parallelism)
that streams files chunk-by-chunk over SFTP, honouring a per-transfer cancel
flag and reporting throttled progress through a `broadcast` channel. The
presentation layer forwards those as `harbor://transfer-event`. Failed or
cancelled transfers can be retried (re-enqueued).

## Concurrency model

- A single Tokio multi-threaded runtime drives everything.
- One russh `Handle` per session; shells and the SFTP subsystem are multiplexed
  channels over it.
- Shared state uses `Arc<dyn Trait>` for services and `tokio::sync::Mutex` for
  the session/shell registries.
- Synchronous, blocking work (keychain access, key parsing, directory scans) is
  dispatched onto blocking threads via `spawn_blocking`.

## Frontend architecture

```
src/
├── components/   reusable UI (NavRail, Terminal, FileBrowser, modals, icons…)
├── pages/        top-level views (Connections, Transfers, Keys, Settings)
├── hooks/        (reserved for cross-cutting hooks)
├── services/     typed Tauri command wrappers (api.ts) + event helpers
├── stores/       Zustand stores (ui, profiles, sessions, transfers, toasts)
├── lib/          formatting + path helpers
└── styles/       design-system CSS (tokens + components, dark/light)
```

The UI is intentionally thin: every backend call goes through `services/api.ts`,
state lives in small Zustand stores, and no business logic is duplicated from the
Rust core.
