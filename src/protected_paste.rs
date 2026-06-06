use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use crate::clipboard::ClipboardMonitor;
use crate::config::{self, Config};
use crate::detector::Detector;
use crate::notify;
use crate::redact_cli;
use crate::stats;
use crate::targets::{self, TargetSurface};

const TARGET_CACHE_FILE: &str = "target-state.json";
const MAX_INPUT_BYTES: usize = 1024 * 1024;
const RESTORE_DELAY: Duration = Duration::from_millis(900);
const AI_PROCESS_SCAN_CACHE_MS: u64 = 2_000;
static SYSTEM_PASTE_BYPASS_UNTIL_MS: AtomicU64 = AtomicU64::new(0);
static AI_PROCESS_CWD_CACHE: OnceLock<Mutex<AiProcessCwdCache>> = OnceLock::new();

#[derive(Debug, Clone, Default)]
struct AiProcessCwdCache {
    updated_at_ms: u64,
    entries: Vec<(String, String)>,
}

#[derive(Debug, Clone, Deserialize)]
struct TargetSnapshot {
    #[serde(default)]
    reason: Option<String>,
    expires_at: u64,
}

#[derive(Debug, Clone, Deserialize)]
struct TerminalTarget {
    kind: String,
    cwd: String,
    #[serde(default)]
    terminal_app: Option<String>,
    #[serde(default)]
    terminal_id: Option<String>,
    expires_at: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum RestoreMode {
    Sync,
    Async,
}

pub struct Engine {
    config: Config,
    detector: Detector,
}

impl Engine {
    pub fn new() -> Self {
        let config = Config::load();
        Self::from_config(config)
    }

    pub fn from_config(config: Config) -> Self {
        let detector = Detector::from_config(&config);
        Self { config, detector }
    }

    pub fn replace_config(&mut self, config: Config) {
        *self = Self::from_config(config);
    }

    pub fn paste_with_cached_target(&mut self, reason: Option<String>) -> anyhow::Result<()> {
        paste_with(reason, &self.config, &self.detector, RestoreMode::Async)
    }

    pub fn paste_force_redact(&mut self) -> anyhow::Result<()> {
        paste_with(
            Some("shortcut:force-redact".to_string()),
            &self.config,
            &self.detector,
            RestoreMode::Sync,
        )
    }
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

pub fn run() -> anyhow::Result<()> {
    let reason = current_target_reason();
    let engine = Engine::new();
    paste_with(reason, &engine.config, &engine.detector, RestoreMode::Sync)
}

fn paste_with(
    reason: Option<String>,
    config: &Config,
    detector: &Detector,
    restore_mode: RestoreMode,
) -> anyhow::Result<()> {
    if !config.beforepaste_enabled {
        paste_debug("passthrough: beforepaste disabled");
        return emit_system_paste();
    }
    let Some(reason) = reason else {
        paste_debug("passthrough: no target");
        return emit_system_paste();
    };
    paste_debug(&format!("start: reason={reason}"));

    let mut monitor = match ClipboardMonitor::new(0) {
        Ok(m) => m,
        Err(error) => {
            paste_debug(&format!("passthrough: clipboard unavailable: {error}"));
            return emit_system_paste();
        }
    };
    let Some(text) = monitor.read_text().filter(|s| !s.is_empty()) else {
        paste_debug("passthrough: clipboard has no text");
        return emit_system_paste();
    };

    if text.len() > MAX_INPUT_BYTES {
        paste_debug(&format!("passthrough: clipboard too large: {}", text.len()));
        return emit_system_paste();
    }

    let (redacted, names) = redact_cli::redact_with(detector, config, &text);
    if names.is_empty() || redacted == text {
        paste_debug(&format!(
            "passthrough: no redaction input_len={} output_len={}",
            text.len(),
            redacted.len()
        ));
        return emit_system_paste();
    }

    paste_debug(&format!(
        "redacting: count={} input_len={} output_len={} names={}",
        names.len(),
        text.len(),
        redacted.len(),
        names.join(", ")
    ));
    monitor.replace_text(&redacted)?;
    let paste_result = emit_system_paste();
    restore_clipboard(text, redacted, restore_mode);
    paste_result?;

    stats::append(names.len() as u64);
    if !config.silent {
        notify::redacted_notification(names.len(), config.notification_timeout_secs, config.lang);
    }
    log::info!(
        "protected paste redacted before {}: {}",
        reason,
        names.join(", ")
    );
    Ok(())
}

fn restore_clipboard(original: String, redacted: String, mode: RestoreMode) {
    let restore = move || {
        std::thread::sleep(RESTORE_DELAY);
        let Ok(mut monitor) = ClipboardMonitor::new(0) else {
            return;
        };
        if monitor.read_text().as_deref() == Some(redacted.as_str()) {
            let _ = monitor.replace_text(&original);
            paste_debug("restore: original clipboard restored");
        } else {
            paste_debug("restore: skipped because clipboard changed");
        }
    };
    match mode {
        RestoreMode::Sync => restore(),
        RestoreMode::Async => {
            std::thread::spawn(restore);
        }
    }
}

fn paste_debug(message: &str) {
    let path = config::ensure_base_dir().join("protected-paste.log");
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    let _ = writeln!(file, "{} {message}", now_secs());
}

pub fn current_target_reason() -> Option<String> {
    detect_current_target().or_else(read_target_cache)
}

fn read_target_cache() -> Option<String> {
    let data = fs::read_to_string(config::base_dir().join(TARGET_CACHE_FILE)).ok()?;
    let snapshot: TargetSnapshot = serde_json::from_str(&data).ok()?;
    if snapshot.expires_at <= now_secs() {
        return None;
    }
    snapshot.reason
}

#[cfg(target_os = "macos")]
fn detect_current_target() -> Option<String> {
    let config = Config::load();
    let bundle = frontmost_bundle()?;
    if let Some(target) = targets::match_macos_bundle(&config, &bundle) {
        return Some(format!("app:{}", target.id));
    }
    if BROWSERS.contains(&bundle.as_str()) {
        let url = active_tab_url(&bundle)?;
        let host = host_of(&url)?;
        if let Some((target, domain)) = targets::match_domain(&config, &host) {
            return Some(format!("web:{}:{domain}", target.id));
        }
        return None;
    }
    if TERMINALS.contains(&bundle.as_str()) {
        if bundle == "com.mitchellh.ghostty" {
            if let Some(terminal_id) = ghostty_focused_terminal_id() {
                if let Some(target) = active_terminal_by_id("ghostty", &terminal_id) {
                    if targets::enabled_on(&config, TargetSurface::Terminal, &target.kind) {
                        return Some(format!("cli:{}", target.kind));
                    }
                }
            }
            if let Some(cwd) = ghostty_focused_working_directory() {
                if let Some(kind) = ai_process_kind_for_cwd(&cwd) {
                    if targets::enabled_on(&config, TargetSurface::Terminal, &kind) {
                        return Some(format!("cli:{kind}"));
                    }
                }
            }
        } else if bundle == "com.googlecode.iterm2" {
            if let Some(tty) = iterm2_current_session_tty() {
                if let Some(target) = read_terminal_target_by_tty(&tty) {
                    if targets::enabled_on(&config, TargetSurface::Terminal, &target.kind) {
                        return Some(format!("cli:{}", target.kind));
                    }
                }
            }
        }
        if let Some(title) = focused_window_title() {
            if let Some(kind) = terminal_ai_cli(&title) {
                if targets::enabled_on(&config, TargetSurface::Terminal, kind) {
                    return Some(format!("cli:{kind}"));
                }
            }
            if let Some(target) = active_terminal_by_title(&title) {
                if targets::enabled_on(&config, TargetSurface::Terminal, &target.kind) {
                    return Some(format!("cli:{}", target.kind));
                }
            }
        }
        return None;
    }
    if VSCODE.contains(&bundle.as_str()) {
        if let Some(target) = active_terminal_by_app("vscode") {
            if targets::enabled_on(&config, TargetSurface::Vscode, &target.kind) {
                return Some(format!("cli:{}", target.kind));
            }
        }
    }
    None
}

#[cfg(not(target_os = "macos"))]
fn detect_current_target() -> Option<String> {
    None
}

#[cfg(target_os = "macos")]
const BROWSERS: &[&str] = &[
    "com.google.Chrome",
    "com.brave.Browser",
    "com.microsoft.edgemac",
    "com.vivaldi.Vivaldi",
    "company.thebrowser.Browser",
    "com.apple.Safari",
];

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
const VSCODE: &[&str] = &[
    "com.microsoft.VSCode",
    "com.microsoft.VSCodeInsiders",
    "com.visualstudio.code.oss",
];

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
fn frontmost_bundle() -> Option<String> {
    osascript(
        "tell application \"System Events\" to get bundle identifier of \
         first application process whose frontmost is true",
    )
}

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
fn focused_window_title() -> Option<String> {
    osascript(
        "tell application \"System Events\" to tell \
         (first process whose frontmost is true) to get title of front window",
    )
}

#[cfg(target_os = "macos")]
fn ghostty_focused_terminal_id() -> Option<String> {
    osascript(
        "tell application \"Ghostty\" to get id of focused terminal of selected tab of front window",
    )
}

#[cfg(target_os = "macos")]
fn ghostty_focused_working_directory() -> Option<String> {
    osascript(
        "tell application \"Ghostty\" to get working directory of focused terminal of selected tab of front window",
    )
}

#[cfg(target_os = "macos")]
fn iterm2_current_session_tty() -> Option<String> {
    osascript("tell application \"iTerm2\" to get tty of current session of current window")
        .or_else(|| {
            osascript(
                "tell application \"iTerm2\" to tell current window to get tty of current session",
            )
        })
}

#[cfg(target_os = "macos")]
fn host_of(url: &str) -> Option<String> {
    let after = url.split("://").nth(1)?;
    let authority = after.split('/').next()?;
    let host = authority.rsplit('@').next()?;
    let host = host.split(':').next()?;
    Some(host.to_lowercase())
}

fn terminal_ai_cli(title: &str) -> Option<&'static str> {
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

#[cfg(target_os = "macos")]
fn ai_process_kind_for_cwd(cwd: &str) -> Option<String> {
    let cwd = normalize_path(cwd);
    if cwd.is_empty() {
        return None;
    }
    let mut kinds = Vec::<String>::new();
    for (kind, process_cwd) in cached_ai_process_cwds() {
        if normalize_path(&process_cwd) == cwd && !kinds.contains(&kind) {
            kinds.push(kind);
        }
    }
    if kinds.len() == 1 {
        kinds.pop()
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
fn cached_ai_process_cwds() -> Vec<(String, String)> {
    let now = now_millis();
    let cache = AI_PROCESS_CWD_CACHE.get_or_init(|| Mutex::new(AiProcessCwdCache::default()));
    let Ok(mut cache) = cache.lock() else {
        return scan_ai_process_cwds();
    };
    if now.saturating_sub(cache.updated_at_ms) >= AI_PROCESS_SCAN_CACHE_MS {
        cache.entries = scan_ai_process_cwds();
        cache.updated_at_ms = now;
    }
    cache.entries.clone()
}

#[cfg(target_os = "macos")]
fn scan_ai_process_cwds() -> Vec<(String, String)> {
    let output = Command::new("/usr/sbin/lsof")
        .args([
            "-a", "-c", "codex", "-c", "gemini", "-c", "claude", "-c", "aider", "-c", "opencode",
            "-d", "cwd", "-Fn", "-Fc",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if output.stdout.is_empty() {
        return Vec::new();
    }
    parse_lsof_process_cwds(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(target_os = "macos")]
fn parse_lsof_process_cwds(output: &str) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    let mut current_kind: Option<String> = None;
    for line in output.lines() {
        if line.is_empty() {
            continue;
        }
        let (tag, value) = line.split_at(1);
        match tag {
            "p" => current_kind = None,
            "c" => current_kind = classify_ai_process_name(value),
            "n" => {
                if let Some(kind) = current_kind.as_ref() {
                    entries.push((kind.clone(), value.to_string()));
                }
            }
            _ => {}
        }
    }
    entries
}

#[cfg(target_os = "macos")]
fn classify_ai_process_name(name: &str) -> Option<String> {
    let normalized = name
        .trim()
        .trim_end_matches(".exe")
        .trim_end_matches(".cmd")
        .trim_end_matches(".bat")
        .to_ascii_lowercase();
    for kind in ["codex", "gemini", "claude", "aider", "opencode"] {
        if normalized == kind
            || normalized
                .strip_prefix(kind)
                .is_some_and(|rest| rest.starts_with('-') || rest.starts_with('_'))
        {
            return Some(kind.to_string());
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn normalize_path(path: &str) -> String {
    let path = path.trim().trim_end_matches('/');
    if path.is_empty() {
        "/".to_string()
    } else {
        path.to_string()
    }
}

fn active_terminal_by_id(terminal_app: &str, terminal_id: &str) -> Option<TerminalTarget> {
    active_terminal(|target| {
        target.terminal_app.as_deref() == Some(terminal_app)
            && target.terminal_id.as_deref() == Some(terminal_id)
    })
}

fn active_terminal_by_app(terminal_app: &str) -> Option<TerminalTarget> {
    active_terminal(|target| target.terminal_app.as_deref() == Some(terminal_app))
}

fn active_terminal_by_title(title: &str) -> Option<TerminalTarget> {
    active_terminal(|target| title_matches_cwd(title, &target.cwd))
}

fn read_terminal_target_by_tty(tty: &str) -> Option<TerminalTarget> {
    read_terminal_target_path(state_path(tty))
}

fn active_terminal(mut predicate: impl FnMut(&TerminalTarget) -> bool) -> Option<TerminalTarget> {
    let entries = fs::read_dir(states_dir()).ok()?;
    let mut matches = Vec::new();
    for entry in entries.flatten() {
        let Some(target) = read_terminal_target_path(entry.path()) else {
            continue;
        };
        if target.expires_at <= now_secs() {
            continue;
        }
        if predicate(&target) {
            matches.push(target);
        }
    }
    if matches.len() == 1 {
        matches.pop()
    } else {
        None
    }
}

fn read_terminal_target_path(path: PathBuf) -> Option<TerminalTarget> {
    let target: TerminalTarget = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
    if target.expires_at <= now_secs() {
        None
    } else {
        Some(target)
    }
}

fn state_path(tty: &str) -> PathBuf {
    states_dir().join(format!("{}.json", state_key(tty)))
}

fn states_dir() -> PathBuf {
    config::base_dir().join("terminal-targets")
}

fn state_key(tty: &str) -> String {
    tty.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn title_matches_cwd(title: &str, cwd: &str) -> bool {
    let title = title.trim();
    if title.is_empty() {
        return false;
    }
    let cwd_name = cwd
        .rsplit('/')
        .find(|part| !part.is_empty())
        .unwrap_or(cwd)
        .trim();
    !cwd_name.is_empty()
        && (title == cwd_name
            || title.ends_with(&format!("({cwd_name})"))
            || title.ends_with(&format!(" {cwd_name}")))
}

#[cfg(target_os = "macos")]
fn emit_system_paste() -> anyhow::Result<()> {
    match emit_applescript_paste() {
        Ok(()) => Ok(()),
        Err(error) => {
            paste_debug(&format!(
                "paste: applescript failed; using cgevent: {error}"
            ));
            arm_system_paste_bypass(Duration::from_millis(750));
            emit_cgevent_paste()
        }
    }
}

pub fn consume_system_paste_bypass() -> bool {
    let until = SYSTEM_PASTE_BYPASS_UNTIL_MS.load(Ordering::SeqCst);
    if until == 0 || until < now_millis() {
        return false;
    }
    SYSTEM_PASTE_BYPASS_UNTIL_MS
        .compare_exchange(until, 0, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
}

fn arm_system_paste_bypass(duration: Duration) {
    let until = now_millis().saturating_add(duration.as_millis() as u64);
    SYSTEM_PASTE_BYPASS_UNTIL_MS.store(until, Ordering::SeqCst);
}

#[cfg(target_os = "macos")]
fn emit_cgevent_paste() -> anyhow::Result<()> {
    use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, KeyCode};
    use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};

    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
        .map_err(|_| anyhow::anyhow!("failed to create CGEventSource"))?;
    let key_down = CGEvent::new_keyboard_event(source.clone(), KeyCode::ANSI_V, true)
        .map_err(|_| anyhow::anyhow!("failed to create paste keydown event"))?;
    let key_up = CGEvent::new_keyboard_event(source, KeyCode::ANSI_V, false)
        .map_err(|_| anyhow::anyhow!("failed to create paste keyup event"))?;
    key_down.set_flags(CGEventFlags::CGEventFlagCommand);
    key_up.set_flags(CGEventFlags::CGEventFlagCommand);
    key_down.post(CGEventTapLocation::HID);
    key_up.post(CGEventTapLocation::HID);
    Ok(())
}

#[cfg(target_os = "macos")]
fn emit_applescript_paste() -> anyhow::Result<()> {
    let script = r#"
tell application "System Events"
  set frontProc to first application process whose frontmost is true
  click menu item "Paste" of menu "Edit" of menu bar 1 of frontProc
end tell
"#;
    let status = Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(script)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("system paste failed with status {status}")
    }
}

#[cfg(target_os = "windows")]
fn emit_system_paste() -> anyhow::Result<()> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_KEYBOARD, KEYEVENTF_KEYUP, VK_CONTROL,
    };

    const VK_V: u16 = 0x56;

    unsafe {
        let mut inputs: [INPUT; 4] = std::mem::zeroed();
        inputs[0].r#type = INPUT_KEYBOARD;
        inputs[0].Anonymous.ki.wVk = VK_CONTROL;

        inputs[1].r#type = INPUT_KEYBOARD;
        inputs[1].Anonymous.ki.wVk = VK_V;

        inputs[2].r#type = INPUT_KEYBOARD;
        inputs[2].Anonymous.ki.wVk = VK_V;
        inputs[2].Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;

        inputs[3].r#type = INPUT_KEYBOARD;
        inputs[3].Anonymous.ki.wVk = VK_CONTROL;
        inputs[3].Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;

        let sent = SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        );
        if sent != inputs.len() as u32 {
            anyhow::bail!(
                "windows paste input injection sent {sent}/{} events",
                inputs.len()
            );
        }
    }

    Ok(())
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn emit_system_paste() -> anyhow::Result<()> {
    anyhow::bail!("protected-paste system paste is not implemented on this OS yet")
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "macos")]
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn parses_ai_process_cwds_from_lsof_output() {
        let output = concat!(
            "p15015\n",
            "ccodex-aarch64-apple-darwin\n",
            "fcwd\n",
            "n/Users/example/code/beforepaste\n",
            "p16372\n",
            "cgemini\n",
            "fcwd\n",
            "n/Users/example/code/demo-project\n",
            "p20000\n",
            "czsh\n",
            "fcwd\n",
            "n/Users/example/code/plain\n",
        );
        assert_eq!(
            parse_lsof_process_cwds(output),
            vec![
                (
                    "codex".to_string(),
                    "/Users/example/code/beforepaste".to_string()
                ),
                (
                    "gemini".to_string(),
                    "/Users/example/code/demo-project".to_string()
                )
            ]
        );
    }
}
