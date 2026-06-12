use std::fs::{self, OpenOptions};
use std::io::Write;
#[cfg(target_os = "macos")]
use std::path::PathBuf;
#[cfg(target_os = "macos")]
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;

#[cfg(target_os = "macos")]
use crate::ai_command;
use crate::clipboard::ClipboardMonitor;
use crate::config::{self, Config};
use crate::detector::Detector;
use crate::notify;
use crate::redact_cli;
use crate::stats;
#[cfg(target_os = "macos")]
use crate::targets::{self, TargetSurface};
#[cfg(target_os = "macos")]
use crate::vscode_surface::{self, VscodeSurface};

const TARGET_CACHE_FILE: &str = "target-state.json";
const MAX_INPUT_BYTES: usize = 1024 * 1024;
const RESTORE_DELAY: Duration = Duration::from_millis(900);
#[cfg(target_os = "macos")]
const AI_PROCESS_SCAN_CACHE_MS: u64 = 2_000;
static SYSTEM_PASTE_BYPASS_UNTIL_MS: AtomicU64 = AtomicU64::new(0);
static RESTORE_GENERATION: AtomicU64 = AtomicU64::new(0);
static PENDING_RESTORE: OnceLock<Mutex<Option<RestoreTicket>>> = OnceLock::new();
#[cfg(target_os = "macos")]
static AI_PROCESS_CWD_CACHE: OnceLock<Mutex<AiProcessCwdCache>> = OnceLock::new();

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Deserialize)]
struct TerminalTarget {
    kind: String,
    cwd: String,
    #[serde(default)]
    terminal_app: Option<String>,
    #[serde(default)]
    terminal_id: Option<String>,
    #[serde(default)]
    vscode_surface: Option<String>,
    expires_at: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum RestoreMode {
    Sync,
    Async,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RestoreTicket {
    generation: u64,
    original: String,
    redacted: String,
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
        return emit_non_target_paste();
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
        if let Some(ticket) = refresh_pending_restore_for_redacted_text(&text) {
            paste_debug(&format!(
                "passthrough: pending redacted clipboard input_len={}",
                text.len()
            ));
            let paste_result = emit_system_paste();
            schedule_clipboard_restore(ticket, restore_mode);
            return paste_result;
        }
        if text_looks_redacted(&text) {
            if let Some(ticket) = rearm_restore_for_redacted_text(&mut monitor, &text)? {
                paste_debug(&format!(
                    "passthrough: rearmed redacted clipboard input_len={}",
                    text.len()
                ));
                let paste_result = emit_system_paste();
                schedule_clipboard_restore(ticket, restore_mode);
                return paste_result;
            }
        }
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
    schedule_clipboard_restore(arm_clipboard_restore(text, redacted), restore_mode);
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

fn emit_non_target_paste() -> anyhow::Result<()> {
    if restore_pending_original_now()? {
        paste_debug("passthrough: restored pending original for non-target");
    }
    emit_system_paste()
}

fn schedule_clipboard_restore(ticket: RestoreTicket, mode: RestoreMode) {
    let restore = move || {
        std::thread::sleep(RESTORE_DELAY);
        let Some(mut pending) = PENDING_RESTORE.get().and_then(|slot| slot.lock().ok()) else {
            return;
        };
        if pending
            .as_ref()
            .is_none_or(|current| current.generation != ticket.generation)
        {
            paste_debug("restore: skipped stale generation");
            return;
        }
        let Ok(mut monitor) = ClipboardMonitor::new(0) else {
            return;
        };
        if monitor.read_text().as_deref() == Some(ticket.redacted.as_str()) {
            let _ = monitor.replace_text(&ticket.original);
            *pending = None;
            paste_debug("restore: original clipboard restored");
        } else {
            *pending = None;
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

fn arm_clipboard_restore(original: String, redacted: String) -> RestoreTicket {
    let generation = RESTORE_GENERATION
        .fetch_add(1, Ordering::SeqCst)
        .saturating_add(1);
    let ticket = RestoreTicket {
        generation,
        original,
        redacted,
    };
    let slot = PENDING_RESTORE.get_or_init(|| Mutex::new(None));
    if let Ok(mut pending) = slot.lock() {
        *pending = Some(ticket.clone());
    }
    ticket
}

fn refresh_pending_restore_for_redacted_text(redacted: &str) -> Option<RestoreTicket> {
    let slot = PENDING_RESTORE.get_or_init(|| Mutex::new(None));
    let mut pending = slot.lock().ok()?;
    let current = pending.as_ref()?;
    if current.redacted != redacted {
        return None;
    }
    let ticket = RestoreTicket {
        generation: RESTORE_GENERATION
            .fetch_add(1, Ordering::SeqCst)
            .saturating_add(1),
        original: current.original.clone(),
        redacted: current.redacted.clone(),
    };
    *pending = Some(ticket.clone());
    Some(ticket)
}

fn rearm_restore_for_redacted_text(
    monitor: &mut ClipboardMonitor,
    redacted: &str,
) -> anyhow::Result<Option<RestoreTicket>> {
    let Some(current) = monitor.read_text() else {
        return Ok(None);
    };
    if current == redacted {
        return Ok(None);
    }
    monitor.replace_text(redacted)?;
    Ok(Some(arm_clipboard_restore(current, redacted.to_string())))
}

fn text_looks_redacted(text: &str) -> bool {
    text.contains("[API_KEY]")
        || text.contains("[OPENAI_API_KEY]")
        || text.contains("[DOTENV_SECRET_LINE]")
        || text.contains("[LABELED_SECRET_LINE]")
        || text.contains("[ALIYUN_ACCESS_KEY_SECRET]")
}

pub fn pending_clipboard_restore_active() -> bool {
    PENDING_RESTORE
        .get()
        .and_then(|slot| slot.lock().ok())
        .and_then(|pending| pending.as_ref().map(|_| ()))
        .is_some()
}

fn restore_pending_original_now() -> anyhow::Result<bool> {
    let Some(slot) = PENDING_RESTORE.get() else {
        return Ok(false);
    };
    let mut pending = match slot.lock() {
        Ok(pending) => pending,
        Err(_) => return Ok(false),
    };
    let Some(ticket) = pending.clone() else {
        return Ok(false);
    };
    let Ok(mut monitor) = ClipboardMonitor::new(0) else {
        return Ok(false);
    };
    if monitor.read_text().as_deref() != Some(ticket.redacted.as_str()) {
        return Ok(false);
    }
    monitor.replace_text(&ticket.original)?;
    *pending = None;
    Ok(true)
}

fn paste_debug(message: &str) {
    let path = config::ensure_base_dir().join("protected-paste.log");
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    let _ = writeln!(file, "{} {message}", now_secs());
}

pub fn current_target_reason() -> Option<String> {
    let detected = detect_current_target();
    if detected.is_some() {
        return detected;
    }

    // The target cache is written by companion watchers and can briefly outlive
    // a focus change. Only trust it while a terminal-like surface is frontmost;
    // otherwise an AI CLI target can leak into Chrome, WeChat, editors, etc.
    #[cfg(target_os = "macos")]
    {
        let bundle = frontmost_bundle()?;
        if TERMINALS.contains(&bundle.as_str()) {
            return read_cli_target_cache();
        }
        if VSCODE.contains(&bundle.as_str()) {
            return match vscode_surface::focused_surface() {
                VscodeSurface::Terminal | VscodeSurface::AiView(_) => read_cli_target_cache(),
                VscodeSurface::Editor | VscodeSurface::Other | VscodeSurface::Unknown => None,
            };
        }
        None
    }

    #[cfg(not(target_os = "macos"))]
    {
        read_target_cache()
    }
}

pub fn current_detected_target_reason() -> Option<String> {
    detect_current_target()
}

#[cfg(target_os = "macos")]
fn read_cli_target_cache() -> Option<String> {
    read_target_cache().filter(|reason| reason.starts_with("cli:"))
}

#[cfg(target_os = "macos")]
pub fn current_target_debug_snapshot() -> String {
    let Some(bundle) = frontmost_bundle() else {
        return "bundle=unknown".to_string();
    };
    if VSCODE.contains(&bundle.as_str()) {
        let surface = vscode_surface::focused_surface().as_debug_label();
        let target = active_vscode_terminal_target()
            .map(|target| target.kind)
            .unwrap_or_else(|| "none".to_string());
        return format!("bundle={bundle} vscode_surface={surface} vscode_terminal_target={target}");
    }
    if bundle != "com.googlecode.iterm2" {
        return format!("bundle={bundle}");
    }

    let session_id = iterm2_current_session_id();
    let tty = iterm2_current_session_tty();
    let tty_kind = tty
        .as_deref()
        .and_then(ai_process_kind_for_tty)
        .unwrap_or_else(|| "none".to_string());
    let session_kind = iterm2_current_session_ai_cli().unwrap_or_else(|| "none".to_string());
    let cwd_kind = iterm2_current_session_working_directory()
        .as_deref()
        .and_then(ai_process_kind_for_cwd)
        .unwrap_or_else(|| "none".to_string());

    format!(
        "bundle={bundle} iterm_session_id={} tty={} tty_kind={tty_kind} session_kind={session_kind} cwd_kind={cwd_kind}",
        session_id.as_deref().unwrap_or("none"),
        tty.as_deref().unwrap_or("none")
    )
}

#[cfg(not(target_os = "macos"))]
pub fn current_target_debug_snapshot() -> String {
    "platform=unsupported".to_string()
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
            if let Some(session_id) = iterm2_current_session_id() {
                if let Some(target) = active_terminal_by_id("iterm2", &session_id) {
                    if targets::enabled_on(&config, TargetSurface::Terminal, &target.kind) {
                        return Some(format!("cli:{}", target.kind));
                    }
                }
            }
            if let Some(tty) = iterm2_current_session_tty() {
                if let Some(target) = read_terminal_target_by_tty(&tty) {
                    if targets::enabled_on(&config, TargetSurface::Terminal, &target.kind) {
                        return Some(format!("cli:{}", target.kind));
                    }
                }
                if let Some(kind) = ai_process_kind_for_tty(&tty) {
                    if targets::enabled_on(&config, TargetSurface::Terminal, &kind) {
                        return Some(format!("cli:{kind}"));
                    }
                }
            }
            if let Some(kind) = iterm2_current_session_ai_cli() {
                if targets::enabled_on(&config, TargetSurface::Terminal, &kind) {
                    return Some(format!("cli:{kind}"));
                }
            }
            if let Some(cwd) = iterm2_current_session_working_directory() {
                if let Some(kind) = ai_process_kind_for_cwd(&cwd) {
                    if targets::enabled_on(&config, TargetSurface::Terminal, &kind) {
                        return Some(format!("cli:{kind}"));
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
        match vscode_surface::focused_surface() {
            VscodeSurface::Editor | VscodeSurface::Other => return None,
            VscodeSurface::AiView(kind) => {
                if targets::enabled_on(&config, TargetSurface::Vscode, &kind) {
                    return Some(format!("cli:{kind}"));
                }
                return None;
            }
            VscodeSurface::Terminal => {
                if let Some(target) = active_vscode_terminal_target() {
                    if targets::enabled_on(&config, TargetSurface::Vscode, &target.kind) {
                        return Some(format!("cli:{}", target.kind));
                    }
                }
            }
            VscodeSurface::Unknown => return None,
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
fn iterm2_current_session_id() -> Option<String> {
    osascript("tell application \"iTerm2\" to get unique id of current session of current window")
        .or_else(|| {
            osascript(
                "tell application \"iTerm2\" to tell current window to get unique id of current session",
            )
        })
}

#[cfg(target_os = "macos")]
fn iterm2_current_session_variable(name: &str) -> Option<String> {
    let escaped = name.replace('\\', "\\\\").replace('"', "\\\"");
    osascript(&format!(
        "tell application \"iTerm2\" to tell current session of current window to get variable \"{escaped}\""
    ))
    .or_else(|| {
        osascript(&format!(
            "tell application \"iTerm2\" to tell current session of current window to get variable named \"{escaped}\""
        ))
    })
}

#[cfg(target_os = "macos")]
fn iterm2_current_session_working_directory() -> Option<String> {
    for name in ["path", "session.path"] {
        if let Some(path) = iterm2_current_session_variable(name).filter(|value| path_like(value)) {
            return Some(path);
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn iterm2_current_session_ai_cli() -> Option<String> {
    for name in [
        "jobName",
        "session.jobName",
        "commandLine",
        "session.commandLine",
        "name",
        "session.name",
    ] {
        let Some(value) = iterm2_current_session_variable(name) else {
            continue;
        };
        if let Some(kind) = ai_command::classify_command_line(&value) {
            return Some(kind.to_string());
        }
        if let Some(kind) = terminal_ai_cli(&value) {
            return Some(kind.to_string());
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn host_of(url: &str) -> Option<String> {
    let after = url.split("://").nth(1)?;
    let authority = after.split('/').next()?;
    let host = authority.rsplit('@').next()?;
    let host = host.split(':').next()?;
    Some(host.to_lowercase())
}

#[cfg(target_os = "macos")]
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
fn ai_process_kind_for_tty(tty: &str) -> Option<String> {
    let tty = tty.rsplit('/').next()?.trim();
    if tty.is_empty() {
        return None;
    }
    let output = Command::new("/bin/ps")
        .args(["-t", tty, "-o", "comm=", "-o", "command="])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() || output.stdout.is_empty() {
        return None;
    }

    let mut kinds = Vec::<String>::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let kind = classify_ai_process_name(line)
            .or_else(|| ai_command::classify_command_line(line).map(str::to_string));
        if let Some(kind) = kind {
            if !kinds.contains(&kind) {
                kinds.push(kind);
            }
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
            "-a", "-c", "codex", "-c", "gemini", "-c", "claude", "-c", "aider", "-c", "continue",
            "-c", "opencode", "-d", "cwd", "-Fn", "-Fc",
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
    ai_command::classify_binary_name(name).map(str::to_string)
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

#[cfg(target_os = "macos")]
fn path_like(value: &str) -> bool {
    let value = value.trim();
    value == "/"
        || value.starts_with("~/")
        || value.starts_with("/Users/")
        || value.starts_with('/')
}

#[cfg(target_os = "macos")]
fn active_terminal_by_id(terminal_app: &str, terminal_id: &str) -> Option<TerminalTarget> {
    active_terminal(|target| {
        target.terminal_app.as_deref() == Some(terminal_app)
            && target.terminal_id.as_deref() == Some(terminal_id)
    })
}

#[cfg(target_os = "macos")]
fn active_vscode_terminal_target() -> Option<TerminalTarget> {
    active_terminal(|target| {
        target.terminal_app.as_deref() == Some("vscode") && !is_vscode_ai_view_target(target)
    })
}

#[cfg(target_os = "macos")]
fn is_vscode_ai_view_target(target: &TerminalTarget) -> bool {
    target.vscode_surface.as_deref() == Some("ai-view")
        || target.terminal_id.as_deref() == Some("ai-view")
}

#[cfg(target_os = "macos")]
fn active_terminal_by_title(title: &str) -> Option<TerminalTarget> {
    active_terminal(|target| title_matches_cwd(title, &target.cwd))
}

#[cfg(target_os = "macos")]
fn read_terminal_target_by_tty(tty: &str) -> Option<TerminalTarget> {
    read_terminal_target_path(state_path(tty))
}

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
fn read_terminal_target_path(path: PathBuf) -> Option<TerminalTarget> {
    let target: TerminalTarget = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
    if target.expires_at <= now_secs() {
        None
    } else {
        Some(target)
    }
}

#[cfg(target_os = "macos")]
fn state_path(tty: &str) -> PathBuf {
    states_dir().join(format!("{}.json", state_key(tty)))
}

#[cfg(target_os = "macos")]
fn states_dir() -> PathBuf {
    config::base_dir().join("terminal-targets")
}

#[cfg(target_os = "macos")]
fn state_key(tty: &str) -> String {
    tty.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
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
    let mut child = Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(script)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let start = std::time::Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if start.elapsed() >= Duration::from_millis(700) {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("system paste timed out");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
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
    use super::*;
    use serial_test::serial;

    fn clear_pending_restore_for_test() {
        RESTORE_GENERATION.store(0, Ordering::SeqCst);
        let slot = PENDING_RESTORE.get_or_init(|| Mutex::new(None));
        *slot.lock().unwrap() = None;
    }

    #[test]
    #[serial]
    fn repeated_redacted_paste_refreshes_restore_generation() {
        clear_pending_restore_for_test();
        let first = arm_clipboard_restore("raw-secret".to_string(), "[API_KEY]".to_string());
        let refreshed = refresh_pending_restore_for_redacted_text("[API_KEY]").unwrap();

        assert!(refreshed.generation > first.generation);
        assert_eq!(refreshed.original, "raw-secret");
        assert_eq!(refreshed.redacted, "[API_KEY]");
        assert!(refresh_pending_restore_for_redacted_text("[OTHER]").is_none());
    }

    #[test]
    fn recognizes_redaction_markers_for_restore_race_guard() {
        assert!(text_looks_redacted("api_key: [API_KEY]"));
        assert!(text_looks_redacted("OPENAI_API_KEY=[OPENAI_API_KEY]"));
        assert!(!text_looks_redacted("plain text"));
    }

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

    #[cfg(target_os = "macos")]
    #[test]
    fn classifies_iterm_session_command_signals() {
        assert_eq!(
            ai_command::classify_command_line("codex resume abc"),
            Some("codex")
        );
        assert_eq!(
            ai_command::classify_command_line("/opt/bin/gemini"),
            Some("gemini")
        );
        assert_eq!(
            ai_command::classify_command_line("env ANTHROPIC_API_KEY=x claude"),
            Some("claude")
        );
        assert_eq!(
            ai_command::classify_command_line("npx -y @openai/codex"),
            Some("codex")
        );
        assert_eq!(
            ai_command::classify_command_line("continue"),
            Some("continue")
        );
        assert_eq!(ai_command::classify_command_line("vim .env"), None);
        assert_eq!(ai_command::classify_command_line("codex-notes.md"), None);
    }
}
