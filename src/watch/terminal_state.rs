use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Context};
use serde::{Deserialize, Serialize};

use crate::config;

const DEFAULT_TTL_SECS: u64 = 12 * 60 * 60;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalTarget {
    pub tty: String,
    pub cmd: String,
    pub kind: String,
    pub cwd: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_app: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terminal_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vscode_window_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vscode_terminal_id: Option<String>,
    pub updated_at: u64,
    pub expires_at: u64,
}

#[derive(Debug, Clone, Default)]
pub struct TerminalIdentity {
    pub terminal_app: Option<String>,
    pub terminal_id: Option<String>,
    pub vscode_window_id: Option<String>,
    pub vscode_terminal_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StateFile {
    pub path: PathBuf,
    pub target: Option<TerminalTarget>,
    pub parse_error: Option<String>,
}

pub fn enter(
    tty: &str,
    cmd: &str,
    cwd: &Path,
    identity: TerminalIdentity,
    ttl_secs: u64,
) -> anyhow::Result<TerminalTarget> {
    let tty = normalize_tty(tty)?;
    let kind = classify_command(cmd).ok_or_else(|| anyhow!("unsupported terminal command"))?;
    let now = now_secs();
    let target = TerminalTarget {
        tty: tty.clone(),
        cmd: cmd.to_string(),
        kind,
        cwd: cwd.display().to_string(),
        terminal_app: clean_optional(identity.terminal_app),
        terminal_id: clean_optional(identity.terminal_id),
        vscode_window_id: clean_optional(identity.vscode_window_id),
        vscode_terminal_id: clean_optional(identity.vscode_terminal_id),
        updated_at: now,
        expires_at: now.saturating_add(ttl_secs.max(1)),
    };
    let bytes = serde_json::to_vec_pretty(&target)?;
    config::atomic_write(&state_path(&tty), &bytes)?;
    Ok(target)
}

pub fn leave(tty: &str) -> anyhow::Result<bool> {
    let tty = normalize_tty(tty)?;
    let path = state_path(&tty);
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

pub fn read(tty: &str) -> anyhow::Result<Option<TerminalTarget>> {
    let tty = normalize_tty(tty)?;
    read_path(&state_path(&tty))
}

pub fn active_for_terminal_title(title: &str) -> anyhow::Result<Option<TerminalTarget>> {
    active_for_identity(|target| title_matches_cwd(title, &target.cwd))
}

pub fn active_for_tty(tty: &str) -> anyhow::Result<Option<TerminalTarget>> {
    read(tty)
}

pub fn active_for_terminal_id(
    terminal_app: &str,
    terminal_id: &str,
) -> anyhow::Result<Option<TerminalTarget>> {
    let terminal_app = terminal_app.trim();
    let terminal_id = terminal_id.trim();
    if terminal_app.is_empty() || terminal_id.is_empty() {
        return Ok(None);
    }
    active_for_identity(|target| {
        target.terminal_app.as_deref() == Some(terminal_app)
            && target.terminal_id.as_deref() == Some(terminal_id)
    })
}

pub fn active_for_terminal_app(terminal_app: &str) -> anyhow::Result<Option<TerminalTarget>> {
    let terminal_app = terminal_app.trim();
    if terminal_app.is_empty() {
        return Ok(None);
    }
    active_for_identity(|target| target.terminal_app.as_deref() == Some(terminal_app))
}

fn active_for_identity(
    mut predicate: impl FnMut(&TerminalTarget) -> bool,
) -> anyhow::Result<Option<TerminalTarget>> {
    let mut matches = Vec::new();
    let dir = states_dir();
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };

    for entry in entries {
        let entry = entry?;
        let Some(target) = read_path(&entry.path())? else {
            continue;
        };
        if predicate(&target) {
            matches.push(target);
        }
    }

    if matches.len() == 1 {
        Ok(matches.pop())
    } else {
        Ok(None)
    }
}

pub fn classify_command(cmd: &str) -> Option<String> {
    for word in cmd.split_whitespace() {
        let word = word.trim_matches(|c: char| matches!(c, '\'' | '"' | '`'));
        if should_skip_word(word) {
            continue;
        }
        let command = word.rsplit('/').next()?;
        return classify_binary_name(command);
    }
    None
}

fn classify_binary_name(command: &str) -> Option<String> {
    let command = command
        .trim()
        .trim_end_matches(".exe")
        .trim_end_matches(".cmd")
        .trim_end_matches(".bat")
        .to_ascii_lowercase();
    if command.contains('.') {
        return None;
    }
    for kind in ["codex", "gemini", "claude", "aider", "continue", "opencode"] {
        if command == kind
            || command
                .strip_prefix(kind)
                .is_some_and(|rest| rest.starts_with('-') || rest.starts_with('_'))
        {
            return Some(kind.to_string());
        }
    }
    None
}

fn should_skip_word(word: &str) -> bool {
    word.contains('=')
        || matches!(
            word,
            "command" | "builtin" | "exec" | "noglob" | "env" | "sudo"
        )
        || word.starts_with('-')
}

fn clean_optional(value: Option<String>) -> Option<String> {
    let value = value?.trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

pub fn default_ttl_secs() -> u64 {
    DEFAULT_TTL_SECS
}

pub fn state_files() -> anyhow::Result<Vec<StateFile>> {
    let dir = states_dir();
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };

    let mut out = Vec::new();
    for entry in entries {
        let path = entry?.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let data = match fs::read_to_string(&path) {
            Ok(data) => data,
            Err(e) => {
                out.push(StateFile {
                    path,
                    target: None,
                    parse_error: Some(e.to_string()),
                });
                continue;
            }
        };
        match serde_json::from_str::<TerminalTarget>(&data) {
            Ok(target) => out.push(StateFile {
                path,
                target: Some(target),
                parse_error: None,
            }),
            Err(e) => out.push(StateFile {
                path,
                target: None,
                parse_error: Some(e.to_string()),
            }),
        }
    }
    Ok(out)
}

pub fn cleanup_state_files(dry_run: bool) -> anyhow::Result<Vec<PathBuf>> {
    let mut removed = Vec::new();
    let now = now_secs();
    for state in state_files()? {
        let should_remove = match &state.target {
            Some(target) => target.expires_at <= now,
            None => state.parse_error.is_some(),
        };
        if should_remove {
            removed.push(state.path.clone());
            if !dry_run {
                let _ = fs::remove_file(&state.path);
            }
        }
    }
    Ok(removed)
}

fn read_path(path: &Path) -> anyhow::Result<Option<TerminalTarget>> {
    let data = match fs::read_to_string(path) {
        Ok(data) => data,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let target: TerminalTarget =
        serde_json::from_str(&data).with_context(|| format!("parse {}", path.display()))?;
    if target.expires_at <= now_secs() {
        let _ = fs::remove_file(path);
        return Ok(None);
    }
    Ok(Some(target))
}

fn title_matches_cwd(title: &str, cwd: &str) -> bool {
    let Some(project) = Path::new(cwd).file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let title = title.trim();
    if title.eq_ignore_ascii_case(project) {
        return true;
    }

    let mut words = title.split_whitespace();
    let Some(first) = words.next() else {
        return false;
    };
    let Some(last) = words.next_back().or_else(|| words.next()) else {
        return false;
    };
    words.next().is_none() && is_braille_spinner(first) && last.eq_ignore_ascii_case(project)
}

fn is_braille_spinner(s: &str) -> bool {
    s.chars()
        .next()
        .is_some_and(|c| ('\u{2800}'..='\u{28ff}').contains(&c))
}

pub(crate) fn states_dir() -> PathBuf {
    config::ensure_base_dir().join("terminal-targets")
}

fn state_path(tty: &str) -> PathBuf {
    states_dir().join(format!("{}.json", state_key(tty)))
}

fn state_key(tty: &str) -> String {
    tty.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn normalize_tty(tty: &str) -> anyhow::Result<String> {
    let tty = tty.trim();
    if tty.is_empty() || tty == "not a tty" {
        return Err(anyhow!("tty is empty"));
    }
    Ok(tty.to_string())
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    struct ConfigHomeGuard {
        saved: Option<std::ffi::OsString>,
    }

    impl ConfigHomeGuard {
        fn set(path: &Path) -> Self {
            let saved = std::env::var_os("BEFOREPASTE_CONFIG_HOME");
            std::env::set_var("BEFOREPASTE_CONFIG_HOME", path);
            Self { saved }
        }
    }

    impl Drop for ConfigHomeGuard {
        fn drop(&mut self) {
            if let Some(saved) = self.saved.take() {
                std::env::set_var("BEFOREPASTE_CONFIG_HOME", saved);
            } else {
                std::env::remove_var("BEFOREPASTE_CONFIG_HOME");
            }
        }
    }

    #[test]
    fn classify_known_ai_commands() {
        assert_eq!(
            classify_command("codex resume abc"),
            Some("codex".to_string())
        );
        assert_eq!(
            classify_command("/opt/bin/gemini"),
            Some("gemini".to_string())
        );
        assert_eq!(
            classify_command("env OPENAI_API_KEY=x codex resume abc"),
            Some("codex".to_string())
        );
        assert_eq!(
            classify_command("sudo -E /opt/bin/claude"),
            Some("claude".to_string())
        );
        assert_eq!(
            classify_command("/opt/bin/codex-aarch64-apple-darwin"),
            Some("codex".to_string())
        );
        assert_eq!(classify_command("claude.exe"), Some("claude".to_string()));
        assert_eq!(classify_command("vim .env"), None);
        assert_eq!(classify_command("codex-notes.md"), None);
    }

    #[test]
    fn state_key_is_path_safe() {
        assert_eq!(state_key("/dev/ttys005"), "_dev_ttys005");
    }

    #[test]
    fn title_matching_accepts_plain_project_or_codex_spinner() {
        let cwd = "/Users/example/code/beforepaste-rs";
        assert!(title_matches_cwd("beforepaste-rs", cwd));
        assert!(title_matches_cwd("⠙ beforepaste-rs", cwd));
    }

    #[test]
    fn title_matching_rejects_editor_and_unrelated_titles() {
        let cwd = "/Users/example/code/beforepaste-rs";
        assert!(!title_matches_cwd("vim .env", cwd));
        assert!(!title_matches_cwd("working notes", cwd));
        assert!(!title_matches_cwd("beforepaste", cwd));
    }

    #[test]
    #[serial]
    fn active_for_terminal_id_matches_exact_identity() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = ConfigHomeGuard::set(dir.path());
        enter(
            "/dev/ttys007",
            "codex",
            Path::new("/tmp/beforepaste-rs"),
            TerminalIdentity {
                terminal_app: Some("ghostty".to_string()),
                terminal_id: Some("terminal-1".to_string()),
                ..TerminalIdentity::default()
            },
            60,
        )
        .unwrap();

        let target = active_for_terminal_id("ghostty", "terminal-1")
            .unwrap()
            .unwrap();
        assert_eq!(target.kind, "codex");
        assert_eq!(target.tty, "/dev/ttys007");
    }

    #[test]
    #[serial]
    fn active_for_terminal_app_requires_unique_match() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = ConfigHomeGuard::set(dir.path());
        for (tty, terminal_id) in [
            ("/dev/ttys001", "vscode-terminal-1"),
            ("/dev/ttys002", "vscode-terminal-2"),
        ] {
            enter(
                tty,
                "codex",
                Path::new("/tmp/beforepaste-rs"),
                TerminalIdentity {
                    terminal_app: Some("vscode".to_string()),
                    terminal_id: Some(terminal_id.to_string()),
                    ..TerminalIdentity::default()
                },
                60,
            )
            .unwrap();
        }

        assert!(active_for_terminal_app("vscode").unwrap().is_none());
    }
}
