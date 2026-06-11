# Roadmap

Harbor today is a complete, working SSH client and SFTP file manager. This
roadmap captures planned refinements and future extensions, roughly in priority
order. Contributions welcome.

## Near term

- **Keyboard shortcuts & command palette** — `Ctrl/Cmd+K` palette, quick-connect,
  tab switching (`Ctrl+1..9`), close tab, focus search.
- **Terminal search & copy-on-select** — in-terminal find, configurable
  copy/paste behaviour, adjustable font size and scrollback.
- **Resumable transfers** — use SFTP ranged writes to resume interrupted
  uploads/downloads instead of restarting.
- **Recursive folder transfers** — upload/download whole directory trees, with
  per-file progress rolled up into one queue item.
- **Transfer conflict handling** — overwrite / skip / rename prompts and a
  "keep both" option.

## Medium term

- **Keyboard-interactive & 2FA** — full support for `keyboard-interactive`
  (OTP/2FA) auth flows (russh already exposes the primitives).
- **SSH config import** — read `~/.ssh/config` to pre-populate profiles
  (Host, HostName, User, Port, IdentityFile, ProxyJump).
- **Jump hosts / ProxyJump** — connect through bastion hosts.
- **Port forwarding** — local, remote and dynamic (SOCKS) tunnels with a small
  manager UI.
- **Key generation** — generate Ed25519/RSA keypairs via `ssh-key` (using only
  the proven library, never custom formats) and copy public keys to servers.
- **Snippets / saved commands** — per-profile command snippets and a startup
  command.

## Longer term

- **Session persistence & reconnect** — auto-reconnect with backoff, and
  optionally reattach via `tmux`/`screen` integration.
- **Sync** — optional encrypted sync of profiles (never secrets) across devices.
- **Theming** — user-defined terminal colour schemes and UI accent colours.
- **Plugins** — a small extension API for custom panels and protocol handlers.
- **Mobile/companion** — the GUI-agnostic `harbor-core` could back a companion
  app or a CLI/TUI.

## Quality & infrastructure

- CI matrix building and testing on Windows, macOS and Linux.
- Signed release artifacts and auto-update.
- Expanded integration tests against a containerised SSH server in CI.
- Accessibility pass (focus management, ARIA, reduced-motion).
- Localisation / i18n.

## Explicitly out of scope

- **SCP** — superseded by SFTP for a file manager (see
  [ARCHITECTURE.md](ARCHITECTURE.md)).
- **Disabling host-key verification** — Harbor will not add an "ignore host key"
  option (see [SECURITY.md](SECURITY.md)).
- **Custom cryptographic primitives** — Harbor will always delegate crypto to
  audited libraries.
