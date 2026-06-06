use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "beforepaste")]
#[command(about = "BeforePaste - Clipboard PII/Secret Redactor")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(arg_required_else_help = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    #[arg(long, global = true)]
    pub log_level: Option<String>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ShellTitleMode {
    /// Preserve the terminal application's own title. This is the default and
    /// avoids hiding Codex/Gemini/Claude status text.
    Preserve,
    /// Continuously write a BeforePaste title marker while an AI CLI runs.
    /// Stronger detection, but it overwrites the AI CLI's title/status.
    Keepalive,
}

impl From<ShellTitleMode> for crate::shell_rc::TitleMode {
    fn from(value: ShellTitleMode) -> Self {
        match value {
            ShellTitleMode::Preserve => crate::shell_rc::TitleMode::Preserve,
            ShellTitleMode::Keepalive => crate::shell_rc::TitleMode::Keepalive,
        }
    }
}

#[derive(Subcommand)]
pub enum Command {
    Init,
    Trigger,
    /// Target-aware paste path for binding to the normal paste shortcut. If
    /// the recent watch cache says the frontmost app is not an AI target, this
    /// immediately performs a normal paste without reading or rewriting the
    /// clipboard. If it is an AI target, it redacts first, pastes, then restores
    /// the original clipboard best-effort.
    ProtectedPaste,
    Menu,
    Status,
    /// Inspect BeforePaste installation/runtime state. Pass --fix to apply
    /// safe cleanup for stale terminal state and orphaned old hook heartbeats.
    Doctor {
        #[arg(long)]
        fix: bool,
        #[arg(long)]
        aggressive: bool,
    },
    /// Clean stale terminal-target state and old shell-hook heartbeat remnants.
    TerminalCleanup {
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        aggressive: bool,
    },
    RegisterShortcut,
    UnregisterShortcut,
    Uninstall,
    Upgrade,
    /// Check the latest GitHub release and notify if a newer version exists.
    /// Designed for unattended runs (cron / systemd timer): silent on success
    /// and on network failure. Pass --auto-install to also swap the binary.
    UpgradeCheck {
        #[arg(long)]
        auto_install: bool,
    },
    /// Read text from stdin, redact secrets/PII using the active config, write
    /// the result to stdout. Designed for shell pipelines and as the engine
    /// behind the `paste-guard` wrapper.
    Redact,
    /// Background target monitor. By default it also auto-redacts the clipboard
    /// while an AI app/site is frontmost. With --target-cache-only it only
    /// publishes the current target for protected-paste/tray use.
    Watch {
        #[arg(long)]
        target_cache_only: bool,
    },
    /// Print the shell integration block for terminal AI target detection.
    /// This is safe to run after init and does not modify shell rc files.
    ShellHook {
        /// Shell to render for: zsh, bash, or fish. Defaults to $SHELL, then zsh.
        #[arg(long)]
        shell: Option<String>,
        /// Terminal title behavior for AI CLI detection.
        #[arg(long, value_enum, default_value_t = ShellTitleMode::Preserve)]
        title_mode: ShellTitleMode,
    },
    /// Internal shell-hook entrypoint: record that this terminal is running an
    /// AI CLI command. Intended for zsh/bash/fish/PowerShell integration; does
    /// not modify aliases.
    TerminalEnter {
        #[arg(long)]
        cmd: String,
        #[arg(long)]
        cwd: PathBuf,
        #[arg(long)]
        tty: String,
        #[arg(long)]
        terminal_app: Option<String>,
        #[arg(long)]
        terminal_id: Option<String>,
        #[arg(long)]
        vscode_window_id: Option<String>,
        #[arg(long)]
        vscode_terminal_id: Option<String>,
        #[arg(long, default_value_t = crate::watch::terminal_state::default_ttl_secs())]
        ttl_secs: u64,
    },
    /// Internal shell-hook entrypoint: clear the AI CLI state for a terminal.
    TerminalLeave {
        #[arg(long)]
        tty: String,
    },
    /// Print the current shell-hook state for a terminal, if any.
    TerminalStatus {
        #[arg(long)]
        tty: String,
    },
    /// Run a child program inside a PTY and redact any bracketed-paste
    /// payloads on the way in. Exits with the child's exit code. Use it to
    /// wrap AI TUIs like Claude Code or Codex so pasted secrets never reach
    /// the prompt.
    PasteGuard {
        /// The child command and its arguments. Pass after `--`.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true, num_args = 0..)]
        argv: Vec<String>,
    },
}
