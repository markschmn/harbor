# Building Harbor

Harbor produces a single native binary plus platform installers via Tauri.

## TL;DR

```bash
npm install
npm run tauri build
```

Artifacts land in `src-tauri/target/release/bundle/`.

## Prerequisites (all platforms)

- [Rust](https://rustup.rs/) (stable)
- [Node.js](https://nodejs.org/) 20+

## Linux

```bash
sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev \
     libayatana-appindicator3-dev librsvg2-dev \
     build-essential pkg-config patchelf
npm install
npm run tauri build
```

Produces, under `src-tauri/target/release/bundle/`:

- `appimage/Harbor_<ver>_amd64.AppImage` — portable, runs on most distros
- `deb/Harbor_<ver>_amd64.deb` — Debian/Ubuntu
- `rpm/Harbor-<ver>-1.x86_64.rpm` — Fedora/RHEL (requires `rpmbuild` installed)

The standalone binary is `src-tauri/target/release/harbor`.

## Windows

> Windows installers **must be built on Windows** — they cannot be
> cross-compiled from Linux/macOS (they need the MSVC toolchain, WebView2 and the
> WiX/NSIS bundlers).

On a Windows machine:

1. Install [Rust](https://rustup.rs/) (the MSVC toolchain) and
   [Node.js](https://nodejs.org/) 20+.
2. Install [WebView2 runtime](https://developer.microsoft.com/microsoft-edge/webview2/)
   (preinstalled on Windows 11).
3. Build:

   ```powershell
   npm install
   npm run tauri build
   ```

Produces, under `src-tauri\target\release\bundle\`:

- `msi\Harbor_<ver>_x64_en-US.msi` — WiX installer
- `nsis\Harbor_<ver>_x64-setup.exe` — NSIS installer

The standalone binary is `src-tauri\target\release\harbor.exe`.

## macOS

```bash
npm install
npm run tauri build            # current arch
# or a specific arch:
npm run tauri build -- --target aarch64-apple-darwin   # Apple Silicon
npm run tauri build -- --target x86_64-apple-darwin    # Intel
```

Produces `src-tauri/target/release/bundle/{dmg,macos}/`.

## CI: build everything automatically

A GitHub Actions workflow at [`.github/workflows/release.yml`](.github/workflows/release.yml)
builds **all** platforms on their native runners:

- **Tag a release** — push a tag like `v0.1.0` and it creates a draft GitHub
  Release with the `.msi`, `.exe`, `.dmg`, `.AppImage`, `.deb` and `.rpm`
  attached.
- **Manual run** — trigger the workflow (`workflow_dispatch`) to download the
  bundles as build artifacts.

This is the recommended way to get signed-ready Windows and macOS binaries
without owning each OS locally.
