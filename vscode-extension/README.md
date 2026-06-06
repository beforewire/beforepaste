# BeforePaste VS Code Extension

This extension is the VS Code terminal identity bridge for BeforePaste desktop
target detection. It does not redact clipboard contents itself. It records when
the active VS Code terminal is running a known AI CLI (`codex`, `gemini`,
`claude`, `opencode`, `aider`, or `continue`) so the tray app can treat that VS
Code terminal as an AI target.

The bridge writes JSON files under the same local BeforePaste config directory:

- macOS: `~/Library/Application Support/beforewire/beforepaste/terminal-targets`
- Windows: `%APPDATA%\\beforewire\\beforepaste\\terminal-targets`
- Linux: `${XDG_CONFIG_HOME:-~/.config}/beforewire/beforepaste/terminal-targets`

Set `BEFOREPASTE_CONFIG_HOME` in the VS Code environment to override the parent
config directory during tests.

## Development

```sh
npm install
npm run compile
```

Then open this folder in VS Code and run the extension host.
