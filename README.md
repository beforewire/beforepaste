# BeforePaste

Local-first paste protection for AI apps, AI websites, and AI terminal workflows.

BeforePaste runs from your menu bar/tray. When you paste into a supported AI
target, it redacts secrets and sensitive values from the clipboard before the
text reaches the target, then restores your original clipboard when it is safe
to do so.

No cloud service. No account. No prompt upload.

See [PRIVACY.md](PRIVACY.md) for the data boundary and telemetry policy.

## Why

AI tools make it easy to paste too much: API keys, `.env` files, cloud
credentials, customer data, shell history, debug logs, and local config snippets.
BeforePaste adds a local safety layer at the point of paste, without asking you
to route prompts through a server.

Use it when you want:

- A menu bar/tray app that protects normal AI paste workflows.
- Positive target detection for AI apps, websites, terminals, and VS Code.
- Local redaction of credentials, tokens, private keys, and common PII.
- A visible Doctor page for runtime status and macOS permission checks.
- CLI tools for scripting, pipelines, and terminal paste-guard experiments.

## Status

BeforePaste is under active development. The macOS desktop tray app is the
primary product path today. The CLI, pipeline redactor, terminal paste guard,
and VS Code terminal bridge are available for development and advanced use.

Platform scope today:

| Platform | Desktop tray | Normal paste protection | Safe paste shortcut | CLI |
|---|---:|---:|---:|---:|
| macOS | Yes | `Cmd+V` in AI targets | `Cmd+Ctrl+V` | Yes |
| Windows | Source/dev only | Not yet | CLI only | Yes |
| Linux | Source/dev only | Not yet | CLI only | Yes |

## Install

### macOS Desktop App

The recommended macOS user path is the desktop app from
[GitHub Releases](https://github.com/beforewire/beforepaste/releases).

Stable latest-download link for the website and docs:

| Platform | Recommended download |
|---|---|
| macOS | [`beforepaste-desktop-macos.dmg`](https://github.com/beforewire/beforepaste/releases/latest/download/beforepaste-desktop-macos.dmg) |

Other assets, ARM builds, and checksums are available from the
[full releases page](https://github.com/beforewire/beforepaste/releases).

Windows and Linux desktop artifacts are paused for the public release. Windows
desktop packaging is still being stabilized, and Linux desktop packaging is
paused until the upstream Tauri Linux GTK dependency chain moves past the
current `glib` advisory. Windows and Linux CLI binaries remain available.

Early macOS preview builds may be unsigned and not notarized. If macOS blocks
the downloaded app, open System Settings -> Privacy & Security and choose
`Open Anyway`, or right-click the app and choose `Open`. For invited testing,
drag `BeforePaste.app` into `/Applications` first. If macOS says the app is
damaged, clear the download quarantine flag and open it again:

```bash
xattr -dr com.apple.quarantine /Applications/BeforePaste.app
open /Applications/BeforePaste.app
```

After changing Accessibility or Input Monitoring permissions, quit and reopen
BeforePaste before checking Doctor again.

If you build locally, the desktop binary is written to:

```text
desktop/src-tauri/target/release/beforepaste-desktop
```

### CLI Downloads

The CLI is available for scripting, CI, and advanced terminal workflows:

| Platform | x86_64 download |
|---|---|
| macOS | [`beforepaste-macos-x86_64`](https://github.com/beforewire/beforepaste/releases/latest/download/beforepaste-macos-x86_64) |
| Windows | [`beforepaste-windows-x86_64.exe`](https://github.com/beforewire/beforepaste/releases/latest/download/beforepaste-windows-x86_64.exe) |
| Linux | [`beforepaste-linux-x86_64`](https://github.com/beforewire/beforepaste/releases/latest/download/beforepaste-linux-x86_64) |

ARM builds are available on the full releases page. The `releases/latest`
links become active after the first public release is published.

### Build From Source

```bash
git clone https://github.com/beforewire/beforepaste.git
cd beforepaste

# Build the CLI.
cargo build --release

# Build the desktop app without producing installers.
cd desktop
npm ci
npm run build:no-bundle
```

Windows and Linux desktop artifacts are paused for the public release. CLI
workflows remain available there; target-aware normal paste protection is
currently macOS-first.

### VS Code Extension

The VS Code extension helps BeforePaste identify AI CLIs running inside VS Code
integrated terminals. It does not redact clipboard contents itself.

Development install from this repo:

```bash
cd vscode-extension
npm ci
npm run compile
npm run package
code --install-extension beforepaste-0.1.0.vsix --force
```

If `code` is not available, run this from VS Code first:

```text
Command Palette -> Shell Command: Install 'code' command in PATH
```

After installing, reload VS Code and run:

```text
Command Palette -> BeforePaste: Show Terminal Bridge Status
```

Then restart `codex`, `claude`, `gemini`, `aider`, `continue`, or `opencode`
inside the integrated terminal so VS Code can publish the active terminal state.

## Quick Start

1. Install and launch the desktop app.
2. Open `Doctor` from the tray and grant the required macOS permissions.
3. Keep `Advanced` mode on for normal `Cmd+V` protection in AI targets.
4. Use `Safe Paste Shortcut Only` if you prefer an explicit protected shortcut.
5. Optional: install the VS Code extension for integrated terminal detection.

Doctor shows permission status and runtime status separately. A permission can
be granted while the selected paste mode is still off or needs attention.

## Paste Modes

| Mode | Shortcut | Behavior |
|---|---|---|
| Advanced | `Cmd+V` on macOS | Intercepts normal paste, checks whether the frontmost target is an AI target, redacts only for AI targets, then pastes. |
| Safe Paste Shortcut Only | `Cmd+Ctrl+V` on macOS | Leaves normal `Cmd+V` alone. The explicit shortcut always pastes a protected copy of the clipboard. |

Advanced mode is the recommended default for macOS because it protects the
normal paste habit in AI targets. Safe Paste Shortcut Only is useful when you
want an explicit action and no normal paste interception.

## Desktop Tray

The tray menu is intentionally small:

- `BeforePaste: ...` shows whether protection is ready.
- `Last target: ...` shows the most recent detected AI target.
- `Protected 24h: ...` shows the rolling 24-hour protection count.
- `Mode` switches between Advanced and Safe Paste Shortcut Only.
- `Preferences` opens settings.
- `Doctor` opens runtime and permission checks.

## macOS Permissions

BeforePaste uses macOS permissions only for local paste protection:

| Permission | Used for |
|---|---|
| Accessibility | Performs the final protected paste action and reads limited UI context needed for target detection. |
| Input Monitoring | Lets Advanced mode observe and intercept normal `Cmd+V`. |
| Automation / App detection access | Reads browser tab URLs and terminal/app context for positive target detection. |

## Target Detection

BeforePaste only protects when it can positively identify an AI target.

Supported target types:

- Native AI apps by bundle identifier, including ChatGPT, Claude Desktop,
  Gemini, and Doubao.
- Browser AI sites by active tab URL, including ChatGPT, Claude, Gemini,
  Doubao, DeepSeek, Kimi, Tongyi, Poe, Perplexity, Copilot, Grok, and related
  domains.
- Terminal AI CLIs, including Codex, Claude, Gemini, aider, Continue, and
  opencode.
- VS Code integrated terminals through the BeforePaste VS Code extension.

Browser matching is positive-only. If BeforePaste cannot read the active tab URL
or cannot confirm an AI target, it performs a normal paste. This avoids
rewriting clipboard content in places like cloud console secret fields, GitHub
Actions secrets, Vercel environment settings, and similar workflows.

For Ghostty on macOS, BeforePaste can also identify terminal targets by matching
the focused terminal pane working directory with a running AI CLI process. Shell
integration and the VS Code extension provide stronger terminal identity when
available.

## Redaction

BeforePaste includes hundreds of built-in patterns across cloud credentials, AI
API keys, developer tokens, payment and SaaS secrets, private key material,
dotenv-style assignments, structured credentials, and common PII formats.

See [DETECTION_COVERAGE.md](DETECTION_COVERAGE.md) for the generated catalog.

Deep scan and entropy scan are available for broader catch-all coverage, but
they are off by default because they trade precision for coverage.

When the redaction style is set to `Typed`, environment-style assignments keep
the variable name and replace only the value:

```bash
export GEMINI_API_KEY="real-secret"
# GEMINI_API_KEY="real-secret"
```

becomes:

```bash
export GEMINI_API_KEY="[GEMINI_API_KEY]"
# GEMINI_API_KEY="[GEMINI_API_KEY]"
```

This also makes repeated protected pastes idempotent: already-redacted values
are not redacted again.

Other redaction styles are available in the CLI/TUI configuration:

| Style | Example output for `aws=AKIAIOSFODNN7EXAMPLE` |
|---|---|
| Marker | `aws=[REDACTED]` |
| Drop | `aws=` |
| Typed | `aws=[AWS_ACCESS_KEY_ID]` |
| Placeholder | `aws=example-redacted-value` using fake sample values |

## CLI

The CLI remains useful for scripting and development:

```bash
# Redact stdin to stdout.
cat secrets.log | beforepaste redact > clean.log

# One-shot local redaction workflow.
beforepaste trigger

# Wrap a terminal AI process and redact bracketed-paste payloads.
beforepaste paste-guard -- codex
```

A background `beforepaste watch` flow is available for advanced use, but the
desktop tray is the preferred user path for normal paste protection.

## Privacy

BeforePaste does not upload clipboard contents, prompts, redaction output,
detected secrets, browser URLs, terminal commands, file paths, or target app
names to a cloud service.

The local activity counter stores timestamp and count only. It does not store
clipboard text, pattern names, target names, or redacted values.

See [PRIVACY.md](PRIVACY.md) for details.

## Development

Common checks:

```bash
cargo fmt --all --check
cargo test

cd desktop
npm run build:no-bundle

cd ../vscode-extension
npm run compile
npm run package
```

The release helper updates app/package versions, creates the release commit,
and tags the release:

```bash
just release
```

GitHub Actions builds CLI and desktop artifacts. Public installer scripts
download from `releases/latest`. CLI update checks query the GitHub Releases API
and verify downloaded assets against the release `SHA256SUMS`.

## Security

Do not open public issues for security problems. See [SECURITY.md](SECURITY.md)
for private reporting instructions.

Do not include real secrets in bug reports, screenshots, reproduction steps, or
missing-pattern examples. Use synthetic values with the same shape instead.

## Attribution

BeforePaste includes code and detection coverage derived from
[secret-stripper](https://github.com/kalix127/secret-stripper) by Gianluca
Iavicoli, licensed under the MIT License. See [LICENSE](LICENSE) and
[NOTICE](NOTICE) for details.

## License

MIT.
