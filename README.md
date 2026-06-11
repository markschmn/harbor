<div align="center">

# ⚓ Harbor

**A modern, secure SSH client and SFTP file manager.**

A clean, native alternative to PuTTY + WinSCP for developers and system administrators —
built with Rust, Tauri v2 and React.

</div>

---

## Why Harbor?

PuTTY and WinSCP are dependable but dated. Harbor brings the same capabilities
into a single, polished desktop app that looks and feels like software from
2026:

- 🔐 **Secure by default** — host keys are always verified, secrets never touch
  plaintext on disk, and all cryptography comes from audited libraries.
- 🖥️ **A real terminal** — full PTY support, resize, UTF-8, copy/paste, multiple
  tabbed sessions, powered by [xterm.js](https://xtermjs.org/).
- 📂 **Dual-pane file manager** — browse local and remote filesystems side by
  side over SFTP, with drag-and-drop transfers and a concurrent transfer queue.
- 🗂️ **Saved connections** — organise hosts with names, tags, favourites and
  notes.
- 🔑 **Key aware** — discovers your `~/.ssh` keys, shows fingerprints, and works
  with the SSH agent, plain keys and encrypted keys.
- 🎨 **Modern UX** — dark/light themes, smooth animations, keyboard-friendly.
- 🦀 **Native performance** — a Rust core with a tiny memory footprint, no
  Electron.

## Feature overview

| Area | Highlights |
|------|------------|
| **Connections** | Create / edit / delete / search / favourite profiles. Fields for name, host, port, user, auth type, key path, notes and tags. |
| **Terminal** | PTY shells, live resize, multiple tabs, UTF‑8, reconnect-aware status, web-link detection. |
| **Files** | Local ⇄ remote dual pane, upload/download, rename, delete, mkdir, drag-and-drop. |
| **Transfers** | Parallel transfers, live progress, history, retry, cancel, error reporting. |
| **Keys** | Discovery from `~/.ssh`, SHA‑256 fingerprints, algorithm/bit display, encrypted-key detection. |
| **Security** | `known_hosts`-backed verification (TOFU), agent/password/key/encrypted-key auth, OS keychain secrets. |

## Tech stack

- **Language:** Rust (stable) + TypeScript
- **Shell:** [Tauri v2](https://v2.tauri.app/) (no Electron)
- **Frontend:** React 19 + Vite, [xterm.js](https://xtermjs.org/), Zustand
- **Async runtime:** Tokio
- **SSH/SFTP:** [`russh`](https://crates.io/crates/russh) + [`russh-sftp`](https://crates.io/crates/russh-sftp) (pure-Rust, see [ARCHITECTURE.md](ARCHITECTURE.md))
- **Crypto/keys:** [`ssh-key`](https://crates.io/crates/ssh-key) (RustCrypto)
- **Secrets:** OS keychain via [`keyring`](https://crates.io/crates/keyring)
- **Config:** TOML + Serde

## Project layout

```
harbor/
├── crates/harbor-core/        # GUI-agnostic core (domain + application + infrastructure)
│   └── src/
│       ├── domain/            # entities, value objects, invariants (no I/O)
│       ├── application/       # use-case services + ports (traits)
│       └── infrastructure/    # russh, russh-sftp, TOML, keychain, known_hosts, keys
├── src-tauri/                 # presentation layer: Tauri commands, state, events
├── src/                       # React frontend (components, pages, hooks, services, stores)
├── ARCHITECTURE.md            # design, SSH library choice, SFTP-vs-SCP rationale
├── SECURITY.md                # security model and threat considerations
└── ROADMAP.md                 # what's next
```

See [ARCHITECTURE.md](ARCHITECTURE.md) for the full design and the rationale
behind the key technical decisions.

## Getting started

### Prerequisites

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 20+
- Platform webview deps for Tauri — see the
  [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/).
  On Debian/Ubuntu:

  ```bash
  sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev \
       libayatana-appindicator3-dev librsvg2-dev build-essential pkg-config
  ```

### Develop

```bash
npm install
npm run tauri dev      # launches the app with hot-reloading frontend
```

### Build a release bundle

```bash
npm run tauri build    # produces a native installer/binary for your platform
```

### Run the test suite

```bash
cargo test             # Rust unit + integration + security tests
npm run typecheck      # TypeScript type checking
```

## Documentation

- **[ARCHITECTURE.md](ARCHITECTURE.md)** — layered design, the russh choice, the SFTP-over-SCP decision.
- **[SECURITY.md](SECURITY.md)** — security model, threat considerations, what Harbor does and doesn't protect against.
- **[ROADMAP.md](ROADMAP.md)** — planned features and future extensions.

## License

[MIT](LICENSE) © Harbor contributors
