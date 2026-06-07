# BeforePaste Desktop

Tauri desktop shell for BeforePaste tray and preferences.

## Development

```sh
cd desktop
npm install
npm run dev
```

The desktop shell reuses the root Rust crate for configuration and detection
primitives. It is intentionally separate from the CLI so command-line workflows
continue to work without Tauri.

## Packaging

The desktop shell is packaged with Tauri. Release CI builds the CLI binaries
from the root crate and the desktop installers from this directory.

```sh
cd desktop
npm ci

# Compile the desktop app without producing installers.
npm run build:no-bundle

# Produce platform installers on the matching host OS.
npm run build:linux     # .deb, .rpm, .AppImage
npm run build:windows   # NSIS .exe, MSI .msi
npm run build:macos     # .app, .dmg

# Regenerate desktop icons after replacing src-tauri/icons/icon.png.
npm run icons
```

Linux packaging requires WebKitGTK, GTK, AppIndicator, librsvg, and patchelf
development packages on the build host. Windows packaging should run on a
Windows runner so Tauri can build NSIS and MSI installers with the native
toolchain.

Release CI produces desktop artifacts for macOS and Windows x86_64/aarch64.
Linux desktop packaging is paused for the public release until the upstream
Tauri Linux GTK dependency chain moves past the current `glib` advisory. The
root CLI release still includes Linux binaries.

Feature scope by platform:

- macOS: tray, preferences, safe paste shortcut, target-aware Cmd+V protection,
  browser/app/terminal detection.
- Windows: tray, preferences, safe paste shortcut, CLI, and generated
  installers. Target-aware normal paste interception remains platform-specific
  work and is currently implemented only on macOS.
- Linux: CLI is published. Desktop source builds remain available for
  development, but public Linux desktop artifacts are paused pending the
  upstream Tauri GTK dependency update.
