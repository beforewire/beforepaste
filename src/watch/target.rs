//! AI-target detection for the `watch` auto-redact loop.
//!
//! Additive fork module (Contextpipe/BeforePaste). Does NOT touch upstream core.
//! macOS: frontmost app bundle id (osascript) + browser active-tab URL (osascript).
//! Fail-safe: a browser whose tab URL can't be read is NOT a target (positive match only).
//! Terminal AI-CLI detection + Windows are later increments.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Detection {
    Ai(String),
    Terminal,
    Other,
}

pub fn current() -> Detection {
    imp::current()
}

#[cfg(target_os = "macos")]
mod imp {
    use super::Detection;
    use crate::config::Config;
    use crate::targets::{self, TargetSurface};
    use crate::watch::terminal_state;
    use std::process::Command;

    const BROWSERS: &[&str] = &[
        "com.google.Chrome",
        "com.brave.Browser",
        "com.microsoft.edgemac",
        "com.vivaldi.Vivaldi",
        "company.thebrowser.Browser",
        "com.apple.Safari",
    ];
    const TERMINALS: &[&str] = &[
        "com.mitchellh.ghostty",
        "com.googlecode.iterm2",
        "com.apple.Terminal",
        "net.kovidgoyal.kitty",
        "com.github.wez.wezterm",
        "dev.warp.Warp-Stable",
        "io.alacritty",
        "co.zeit.hyper",
    ];
    const VSCODE: &[&str] = &[
        "com.microsoft.VSCode",
        "com.microsoft.VSCodeInsiders",
        "com.visualstudio.code.oss",
    ];

    fn osascript(src: &str) -> Option<String> {
        let out = Command::new("/usr/bin/osascript")
            .arg("-e")
            .arg(src)
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    }

    fn frontmost_bundle() -> Option<String> {
        osascript(
            "tell application \"System Events\" to get bundle identifier of \
             first application process whose frontmost is true",
        )
    }

    fn active_tab_url(bundle: &str) -> Option<String> {
        let src = if bundle == "com.apple.Safari" {
            "tell application id \"com.apple.Safari\" to return URL of front document".to_string()
        } else {
            format!(
                "tell application id \"{}\" to return URL of active tab of front window",
                bundle
            )
        };
        osascript(&src)
    }

    /// Focused window title of the frontmost app, via System Events (reuses the
    /// automation grant we already have). For terminals this is the focused tab's
    /// title, which is pane-specific — so vim in another pane is never matched.
    fn focused_window_title() -> Option<String> {
        osascript(
            "tell application \"System Events\" to tell \
             (first process whose frontmost is true) to get title of front window",
        )
    }

    fn ghostty_focused_terminal_id() -> Option<String> {
        osascript(
            "tell application \"Ghostty\" to get id of focused terminal of selected tab of front window",
        )
    }

    fn iterm2_current_session_tty() -> Option<String> {
        osascript("tell application \"iTerm2\" to get tty of current session of current window")
            .or_else(|| {
                osascript(
                    "tell application \"iTerm2\" to tell current window to get tty of current session",
                )
            })
    }

    /// Known AI CLIs set focused terminal titles that are much more specific
    /// than a normal shell/editor title. Keep this positive-only: a generic
    /// terminal stays manual-only unless the focused pane advertises an AI TUI.
    pub(super) fn terminal_ai_cli(title: &str) -> Option<&'static str> {
        let normalized = title.trim().to_ascii_lowercase();

        if let Some(rest) = normalized.strip_prefix("beforepaste:") {
            if let Some((kind, _)) = rest.split_once(':') {
                return match kind {
                    "codex" => Some("codex"),
                    "gemini" => Some("gemini"),
                    "claude" => Some("claude"),
                    "aider" => Some("aider"),
                    "continue" => Some("continue"),
                    "opencode" => Some("opencode"),
                    _ => None,
                };
            }
        }
        let codex_status_title = normalized.starts_with('◇') || normalized.starts_with('✦');
        let codex_status = normalized.trim_start_matches(['◇', '✦']).trim_start();
        if normalized == "codex"
            || normalized.starts_with("codex ")
            || normalized.contains("] action required |")
            || normalized.contains("] working |")
            || (codex_status_title
                && (codex_status.starts_with("ready") || codex_status.starts_with("working")))
        {
            return Some("codex");
        }
        if normalized == "gemini" || normalized.starts_with("gemini ") {
            return Some("gemini");
        }
        let claude_title = normalized
            .strip_prefix('\u{2733}')
            .unwrap_or(&normalized)
            .trim_start();
        if claude_title == "claude" || claude_title.starts_with("claude ") {
            return Some("claude");
        }
        None
    }

    fn host_of(url: &str) -> Option<String> {
        let after = url.split("://").nth(1)?;
        let authority = after.split('/').next()?;
        let host = authority.rsplit('@').next()?; // strip userinfo
        let host = host.split(':').next()?; // strip port
        Some(host.to_lowercase())
    }

    pub fn current() -> Detection {
        let config = Config::load();
        let Some(bundle) = frontmost_bundle() else {
            return Detection::Other;
        };
        if let Some(target) = targets::match_macos_bundle(&config, &bundle) {
            return Detection::Ai(format!("app:{}", target.id));
        }
        if BROWSERS.contains(&bundle.as_str()) {
            let Some(url) = active_tab_url(&bundle) else {
                return Detection::Other;
            }; // fail-safe: no URL -> not a target
            let Some(host) = host_of(&url) else {
                return Detection::Other;
            };
            if let Some((target, domain)) = targets::match_domain(&config, &host) {
                return Detection::Ai(format!("web:{}:{domain}", target.id));
            }
            return Detection::Other;
        }
        if TERMINALS.contains(&bundle.as_str()) {
            if bundle == "com.mitchellh.ghostty" {
                if let Some(terminal_id) = ghostty_focused_terminal_id() {
                    if let Ok(Some(target)) =
                        terminal_state::active_for_terminal_id("ghostty", &terminal_id)
                    {
                        if targets::enabled_on(&config, TargetSurface::Terminal, &target.kind) {
                            return Detection::Ai(format!("cli:{}", target.kind));
                        }
                    }
                }
            } else if bundle == "com.googlecode.iterm2" {
                if let Some(tty) = iterm2_current_session_tty() {
                    if let Ok(Some(target)) = terminal_state::active_for_tty(&tty) {
                        if targets::enabled_on(&config, TargetSurface::Terminal, &target.kind) {
                            return Detection::Ai(format!("cli:{}", target.kind));
                        }
                    }
                }
            }
            // Focused-pane signal only: a terminal whose focused window title shows
            // a known AI-CLI signature. vim/.env panes never match.
            if let Some(title) = focused_window_title() {
                if let Some(cli) = terminal_ai_cli(&title) {
                    if targets::enabled_on(&config, TargetSurface::Terminal, cli) {
                        return Detection::Ai(format!("cli:{cli}"));
                    }
                }
                if let Ok(Some(target)) = terminal_state::active_for_terminal_title(&title) {
                    if targets::enabled_on(&config, TargetSurface::Terminal, &target.kind) {
                        return Detection::Ai(format!("cli:{}", target.kind));
                    }
                }
            }
            return Detection::Terminal;
        }
        if VSCODE.contains(&bundle.as_str()) {
            if let Ok(Some(target)) = terminal_state::active_for_terminal_app("vscode") {
                if targets::enabled_on(&config, TargetSurface::Vscode, &target.kind) {
                    return Detection::Ai(format!("cli:{}", target.kind));
                }
            }
            return Detection::Other;
        }
        Detection::Other
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::imp::terminal_ai_cli;

    #[test]
    fn detects_claude_title_signature() {
        assert_eq!(terminal_ai_cli("✳ Claude"), Some("claude"));
        assert_eq!(terminal_ai_cli("Claude"), Some("claude"));
    }

    #[test]
    fn detects_codex_title_signature() {
        assert_eq!(terminal_ai_cli("◇  Ready (aiinfra)"), Some("codex"));
        assert_eq!(terminal_ai_cli("✦  Working… (aiinfra)"), Some("codex"));
        assert_eq!(
            terminal_ai_cli("[ . ] Action Required | aiinfra"),
            Some("codex")
        );
        assert_eq!(
            terminal_ai_cli("[ ! ] Action Required | aiinfra"),
            Some("codex")
        );
        assert_eq!(terminal_ai_cli("[ * ] Working | aiinfra"), Some("codex"));
        assert_eq!(terminal_ai_cli("codex resume 019e8c34"), Some("codex"));
    }

    #[test]
    fn detects_gemini_title_signature() {
        assert_eq!(terminal_ai_cli("Gemini"), Some("gemini"));
        assert_eq!(terminal_ai_cli("gemini - aiinfra"), Some("gemini"));
    }

    #[test]
    fn detects_shell_hook_title_marker() {
        assert_eq!(terminal_ai_cli("beforepaste:codex:aiinfra"), Some("codex"));
        assert_eq!(
            terminal_ai_cli("beforepaste:gemini:beforepaste-rs"),
            Some("gemini")
        );
        assert_eq!(terminal_ai_cli("beforepaste:unknown:aiinfra"), None);
    }

    #[test]
    fn normal_terminal_title_is_not_ai_target() {
        assert_eq!(terminal_ai_cli("⠴ aiinfra"), None);
        assert_eq!(terminal_ai_cli("✳ aiinfra"), None);
        assert_eq!(terminal_ai_cli("vim .env"), None);
        assert_eq!(terminal_ai_cli("vim gemini.env"), None);
        assert_eq!(terminal_ai_cli("vim codex-notes.md"), None);
        assert_eq!(terminal_ai_cli("working notes"), None);
        assert_eq!(terminal_ai_cli("zsh"), None);
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::Detection;

    // TODO(increment 2+): Windows (GetForegroundWindow) + Linux target detection.
    pub fn current() -> Detection {
        Detection::Other
    }
}
