#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "macos")]
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
#[cfg(target_os = "macos")]
use std::time::Instant;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs, fs::OpenOptions, io::Write};
use std::{path::PathBuf, process::Command};

use beforepaste::config::{self, Config};
use beforepaste::detector::Detector;
use beforepaste::lang::Lang;
use beforepaste::protected_paste;
use beforepaste::redact_cli;
use beforepaste::stats;
use beforepaste::targets::{self, CliTargetCatalogEntry, TargetCatalogEntry};
use beforepaste::{notify, updater};
use serde::{Deserialize, Serialize};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::Emitter;
use tauri::Manager;
use tauri::State;
use tauri::WindowEvent;
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tauri_plugin_global_shortcut::ShortcutState;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TargetSnapshot {
    reason: Option<String>,
    updated_at: u64,
    expires_at: u64,
}

#[derive(Debug, Clone, Serialize)]
struct UpdateStatus {
    available: bool,
    skipped: bool,
    version: Option<String>,
    current_version: Option<String>,
    body: Option<String>,
    html_url: Option<String>,
    download_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct PermissionStatus {
    accessibility: bool,
    input_monitoring: bool,
    event_posting: bool,
    automation: bool,
}

#[derive(Debug, Clone, Serialize)]
struct VscodeBridgeStatus {
    installed: bool,
    install_command: String,
    vsix_path: Option<String>,
    message: String,
}

#[derive(Debug, Clone, Serialize)]
struct TestPayloadStatus {
    source: String,
    redacted: String,
    names: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct LastProtectedPaste {
    updated_at: u64,
    result: String,
    message: String,
}

#[derive(Clone)]
struct TrayMenuState {
    status: MenuItem<tauri::Wry>,
    stats: MenuItem<tauri::Wry>,
}

#[derive(Clone, Default, PartialEq, Eq)]
struct TrayLabels {
    status: String,
    stats: String,
}

#[derive(Clone)]
struct TrayStatsCache {
    updated_at: u64,
    baseline_total: u64,
    label: String,
}

impl Default for TrayStatsCache {
    fn default() -> Self {
        Self {
            updated_at: 0,
            baseline_total: stats::read_buckets().total,
            label: "Protected since launch: 0".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct RuntimeStatus {
    platform: String,
    permissions: PermissionStatus,
    beforepaste_enabled: bool,
    protect_normal_paste: bool,
    normal_paste_event_tap_started: bool,
    normal_paste_event_tap_installed: bool,
    force_paste_hotkey: String,
    force_paste_hotkey_registered: bool,
    current_target: Option<String>,
    last_protected_paste: Option<LastProtectedPaste>,
}

struct AppState {
    engine: Arc<Mutex<protected_paste::Engine>>,
    target: Arc<Mutex<Option<String>>>,
    force_paste_hotkey: Mutex<String>,
    beforepaste_enabled: AtomicBool,
    protect_normal_paste: AtomicBool,
    normal_paste_event_tap_started: AtomicBool,
    normal_paste_event_tap_installed: AtomicBool,
    terminal_frontmost: AtomicBool,
    vscode_frontmost: AtomicBool,
    vscode_editor_frontmost: AtomicBool,
    tray_menu: Mutex<Option<TrayMenuState>>,
    tray_labels: Mutex<TrayLabels>,
    tray_stats: Mutex<TrayStatsCache>,
    target_monitor_label: Mutex<String>,
    paste_test_target_active: AtomicBool,
}

const TEST_PAYLOAD: &str = r#"BeforePaste test sample
model: demo-model
base_url: https://example.invalid/v1
api_key: sk-beforepaste-demo-123456
export ALIYUN_ACCESS_KEY_SECRET=beforepasteDemoSecret
"#;

#[tauri::command]
fn get_config() -> Config {
    Config::load()
}

#[tauri::command]
fn copy_test_payload() -> Result<(), String> {
    let mut clipboard = beforepaste::clipboard::ClipboardMonitor::new(0)
        .map_err(|e| format!("clipboard unavailable: {e}"))?;
    clipboard
        .replace_text(TEST_PAYLOAD)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_test_payload_status() -> TestPayloadStatus {
    let config = Config::load();
    let detector = Detector::from_config(&config);
    let (redacted, names) = redact_cli::redact_with(&detector, &config, TEST_PAYLOAD);
    TestPayloadStatus {
        source: TEST_PAYLOAD.to_string(),
        redacted,
        names,
    }
}

#[tauri::command]
fn set_paste_test_target(active: bool, state: State<'_, Arc<AppState>>) {
    state
        .paste_test_target_active
        .store(active, Ordering::SeqCst);
}

#[tauri::command]
fn get_target_catalog() -> Vec<TargetCatalogEntry> {
    targets::catalog().to_vec()
}

#[tauri::command]
fn get_cli_target_catalog() -> Vec<CliTargetCatalogEntry> {
    targets::cli_catalog().to_vec()
}

#[tauri::command]
fn get_permission_status() -> PermissionStatus {
    permission_status()
}

#[tauri::command]
fn get_vscode_bridge_status(app: tauri::AppHandle) -> VscodeBridgeStatus {
    vscode_bridge_status(Some(&app))
}

#[tauri::command]
fn install_vscode_bridge(app: tauri::AppHandle) -> Result<VscodeBridgeStatus, String> {
    let status = vscode_bridge_status(Some(&app));
    let Some(vsix) = status.vsix_path.as_deref() else {
        return Err("BeforePaste VS Code extension package was not found.".to_string());
    };
    let Some(code) = find_code_cli() else {
        return Err(
            "VS Code 'code' command was not found. Install it from VS Code Command Palette first."
                .to_string(),
        );
    };
    let output = Command::new(code)
        .arg("--install-extension")
        .arg(vsix)
        .arg("--force")
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if stderr.is_empty() {
            "VS Code extension install command failed.".to_string()
        } else {
            stderr
        });
    }
    Ok(vscode_bridge_status(Some(&app)))
}

#[tauri::command]
fn open_privacy_settings(app: tauri::AppHandle, kind: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let url = match kind.as_str() {
            "accessibility" => {
                let _ = request_accessibility_trust();
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
            }
            "input_monitoring" => {
                let _ = request_input_monitoring_trust_on_main_thread(&app);
                "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"
            }
            "automation" => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Automation"
            }
            _ => "x-apple.systempreferences:com.apple.preference.security?Privacy",
        };
        Command::new("/usr/bin/open")
            .arg(url)
            .status()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = kind;
        Err("Privacy settings shortcut is only available on macOS.".to_string())
    }
}

#[tauri::command]
fn reset_macos_permissions() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        const BUNDLE_ID: &str = "com.beforewire.beforepaste";
        const SERVICES: &[&str] = &["Accessibility", "ListenEvent", "PostEvent", "AppleEvents"];

        let mut reset_count = 0usize;
        let mut errors = Vec::new();
        for service in SERVICES {
            match Command::new("/usr/bin/tccutil")
                .args(["reset", service, BUNDLE_ID])
                .output()
            {
                Ok(output) if output.status.success() => {
                    reset_count += 1;
                }
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    let detail = if stderr.is_empty() {
                        format!("tccutil reset {service} exited with {}", output.status)
                    } else {
                        format!("{service}: {stderr}")
                    };
                    errors.push(detail);
                }
                Err(error) => errors.push(format!("{service}: {error}")),
            }
        }

        if !errors.is_empty() {
            desktop_debug(&format!(
                "permission reset warnings after {reset_count} successful services: {}",
                errors.join("; ")
            ));
        }

        if reset_count > 0 {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("macOS permission reset is only available on macOS.".to_string())
    }
}

#[tauri::command]
fn get_runtime_status(app: tauri::AppHandle, state: State<'_, Arc<AppState>>) -> RuntimeStatus {
    runtime_status(&app, state.inner())
}

fn runtime_status(app: &tauri::AppHandle, state: &Arc<AppState>) -> RuntimeStatus {
    let force_paste_hotkey = state
        .force_paste_hotkey
        .lock()
        .ok()
        .map(|value| value.clone())
        .unwrap_or_else(|| Config::load().force_paste_hotkey);
    let force_paste_hotkey_registered = !force_paste_hotkey.is_empty()
        && app
            .global_shortcut()
            .is_registered(force_paste_hotkey.as_str());
    let current_target = state
        .target
        .lock()
        .ok()
        .and_then(|target| target.clone())
        .or_else(protected_paste::current_target_reason)
        .or_else(|| paste_test_target_reason(state));

    RuntimeStatus {
        platform: platform_name().to_string(),
        permissions: permission_status(),
        beforepaste_enabled: state.beforepaste_enabled.load(Ordering::SeqCst),
        protect_normal_paste: state.protect_normal_paste.load(Ordering::SeqCst),
        normal_paste_event_tap_started: state.normal_paste_event_tap_started.load(Ordering::SeqCst),
        normal_paste_event_tap_installed: state
            .normal_paste_event_tap_installed
            .load(Ordering::SeqCst),
        force_paste_hotkey,
        force_paste_hotkey_registered,
        current_target,
        last_protected_paste: read_last_protected_paste(),
    }
}

fn platform_name() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "macos"
    }
    #[cfg(target_os = "windows")]
    {
        "windows"
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        "linux"
    }
}

#[tauri::command]
fn save_config(
    app: tauri::AppHandle,
    config: Config,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let protect_normal_paste = config.protect_normal_paste;
    let beforepaste_enabled = config.beforepaste_enabled;
    let launch_at_login = config.launch_at_login;
    let force_paste_hotkey = config.force_paste_hotkey.clone();
    update_force_paste_shortcut(&app, state.inner(), &force_paste_hotkey)
        .map_err(|e| e.to_string())?;
    sync_launch_at_login(launch_at_login).map_err(|e| e.to_string())?;
    config.save().map_err(|e| e.to_string())?;
    state
        .engine
        .lock()
        .map_err(|_| "engine cache lock poisoned".to_string())?
        .replace_config(config);
    state
        .protect_normal_paste
        .store(protect_normal_paste, Ordering::SeqCst);
    state
        .beforepaste_enabled
        .store(beforepaste_enabled, Ordering::SeqCst);
    desktop_debug(&format!(
        "save_config protect_normal_paste={protect_normal_paste}"
    ));
    if protect_normal_paste {
        #[cfg(target_os = "macos")]
        {
            let _ = request_input_monitoring_trust_on_main_thread(&app);
        }
        ensure_normal_paste_event_tap(state.inner().clone()).map_err(|e| e.to_string())?;
    }
    let _ = app.emit("beforepaste-config-updated", ());
    Ok(())
}

#[allow(dead_code)]
fn set_normal_paste_mode(
    app: &tauri::AppHandle,
    state: Arc<AppState>,
    protect: bool,
) -> Result<(), String> {
    let mut config = Config::load();
    config.protect_normal_paste = protect && cfg!(target_os = "macos");
    let protect_normal_paste = config.protect_normal_paste;
    config.save().map_err(|e| e.to_string())?;
    state
        .engine
        .lock()
        .map_err(|_| "engine cache lock poisoned".to_string())?
        .replace_config(config);
    state
        .protect_normal_paste
        .store(protect_normal_paste, Ordering::SeqCst);
    if protect_normal_paste {
        #[cfg(target_os = "macos")]
        {
            let _ = request_input_monitoring_trust_on_main_thread(app);
        }
        ensure_normal_paste_event_tap(Arc::clone(&state)).map_err(|e| e.to_string())?;
    }
    schedule_tray_status_update(app.clone(), Arc::clone(&state), Duration::from_millis(150));
    let _ = app.emit("beforepaste-config-updated", ());
    Ok(())
}

#[tauri::command]
fn set_manual_target(kind: String) -> Result<(), String> {
    if !targets::cli_catalog()
        .iter()
        .any(|target| target.id == kind)
    {
        return Err(format!("unsupported target kind: {kind}"));
    }
    write_target_snapshot(Some(format!("cli:{kind}")), 30 * 60).map_err(|e| e.to_string())
}

#[tauri::command]
fn clear_manual_target() -> Result<(), String> {
    write_target_snapshot(None, 1).map_err(|e| e.to_string())
}

#[tauri::command]
fn check_for_update() -> Result<UpdateStatus, String> {
    let config = Config::load();
    let info = updater::latest_release_info().map_err(|e| e.to_string())?;
    record_seen_update_version(&info.tag);
    Ok(update_status_from_info(&info, &config))
}

#[tauri::command]
fn skip_update_version(version: String) -> Result<Config, String> {
    let version = version.trim();
    if version.is_empty() {
        return Err("version is empty".to_string());
    }
    let mut config = Config::load();
    config.skip_version = Some(version.to_string());
    config.save().map_err(|e| e.to_string())?;
    Ok(config)
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    let allowed = [
        "https://github.com/beforewire/beforepaste/",
        "https://beforepaste.com/",
    ];
    if !allowed.iter().any(|prefix| url.starts_with(prefix)) {
        return Err("unsupported URL".to_string());
    }
    open_external_url(&url)
}

#[tauri::command]
fn open_logs() -> Result<(), String> {
    let dir = config::ensure_base_dir();
    let log_path = dir.join("desktop.log");
    open_path_external(if log_path.exists() { log_path } else { dir })
}

#[tauri::command]
fn copy_diagnostic_summary(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let config = Config::load();
    let status = runtime_status(&app, state.inner());
    let accessibility = status.permissions.accessibility;
    let shortcut = status.force_paste_hotkey_registered;
    let redaction_style = format!("{:?}", config.redact_style);
    let summary = format!(
        "BeforePaste diagnostic summary\nversion: {}\nplatform: {}\nbeforepaste_enabled: {}\nnormal_paste: unchanged\nsafe_paste_hotkey: {}\nsafe_paste_registered: {}\naccessibility: {}\nredaction_style: {}\nlaunch_at_login: {}\nconfig_dir: {}\n",
        updater::current_version(),
        status.platform,
        status.beforepaste_enabled,
        display_hotkey(&status.force_paste_hotkey),
        shortcut,
        accessibility,
        redaction_style,
        config.launch_at_login,
        config::base_dir().display(),
    );
    let mut clipboard = beforepaste::clipboard::ClipboardMonitor::new(0)
        .map_err(|e| format!("clipboard unavailable: {e}"))?;
    clipboard.replace_text(&summary).map_err(|e| e.to_string())
}

fn open_path_external(path: PathBuf) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("/usr/bin/open");
        command.arg(path);
        command
    };
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("explorer.exe");
        command.arg(path);
        command
    };
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(path);
        command
    };

    command.status().map_err(|e| e.to_string())?;
    Ok(())
}

fn update_status_from_info(info: &updater::LatestReleaseInfo, config: &Config) -> UpdateStatus {
    let skipped = config.skip_version.as_deref() == Some(info.tag.as_str());
    UpdateStatus {
        available: info.available,
        skipped,
        version: Some(info.tag.clone()),
        current_version: Some(updater::current_version().to_string()),
        body: info.body.clone(),
        html_url: info.html_url.clone(),
        download_url: info
            .desktop_download_url
            .clone()
            .or_else(|| info.html_url.clone()),
    }
}

fn record_seen_update_version(tag: &str) {
    let mut config = Config::load();
    if config.last_seen_version.as_deref() == Some(tag) {
        return;
    }
    config.last_seen_version = Some(tag.to_string());
    if let Err(error) = config.save() {
        desktop_debug(&format!("record update version failed: {error}"));
    }
}

fn start_update_check(app: tauri::AppHandle) {
    let config = Config::load();
    if !config.check_for_updates {
        return;
    }
    thread::spawn(move || match updater::latest_release_info() {
        Ok(info) => {
            let previous_seen = config.last_seen_version.clone();
            let status = update_status_from_info(&info, &config);
            record_seen_update_version(&info.tag);
            if info.available
                && !status.skipped
                && previous_seen.as_deref() != Some(info.tag.as_str())
            {
                notify::update_available_notification(
                    config.notification_timeout_secs,
                    config.lang,
                    updater::current_version(),
                    &info.tag,
                );
            }
            let _ = app.emit("beforepaste-update-status", status);
        }
        Err(error) => desktop_debug(&format!("update check failed: {error}")),
    });
}

fn open_external_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut cmd = Command::new("/usr/bin/open");
        cmd.arg(url);
        cmd
    };

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", "start", "", url]);
        cmd
    };

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let mut command = {
        let mut cmd = Command::new("xdg-open");
        cmd.arg(url);
        cmd
    };

    command
        .status()
        .map_err(|e| e.to_string())
        .and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err(format!("open URL exited with {status}"))
            }
        })
}

fn write_target_snapshot(reason: Option<String>, ttl_secs: u64) -> anyhow::Result<()> {
    let now = now_secs();
    let snapshot = TargetSnapshot {
        reason,
        updated_at: now,
        expires_at: now.saturating_add(ttl_secs.max(1)),
    };
    let path = config::base_dir().join("target-state.json");
    config::atomic_write(&path, &serde_json::to_vec_pretty(&snapshot)?)?;
    Ok(())
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn desktop_debug(message: &str) {
    let path = config::ensure_base_dir().join("desktop.log");
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    let _ = writeln!(file, "{} {message}", now_secs());
}

fn show_preferences_panel(app: &tauri::AppHandle, panel: &str) -> bool {
    if let Some(window) = app.get_webview_window("main") {
        #[cfg(target_os = "macos")]
        let activation = app.set_activation_policy(tauri::ActivationPolicy::Regular);
        let before_visible = window.is_visible();
        let before_minimized = window.is_minimized();
        let before_pos = window.outer_position();
        let before_size = window.outer_size();
        let unminimize = window.unminimize();
        let cursor = window.cursor_position();
        let primary_monitor = window.primary_monitor().ok().flatten();
        let cursor_monitor = cursor.as_ref().ok().and_then(|position| {
            window
                .monitor_from_point(position.x, position.y)
                .ok()
                .flatten()
        });
        let current_monitor = window.current_monitor().ok().flatten();
        let monitor = primary_monitor
            .as_ref()
            .or(cursor_monitor.as_ref())
            .or(current_monitor.as_ref());
        let monitor_label = monitor.as_ref().map(|monitor| {
            format!(
                "name={:?} work_area={:?}",
                monitor.name(),
                monitor.work_area()
            )
        });
        let size_for_place = window.outer_size();
        let place = match (monitor.as_ref(), size_for_place.as_ref()) {
            (Some(monitor), Ok(size)) => {
                let area = monitor.work_area();
                let area_width = area.size.width as i32;
                let area_height = area.size.height as i32;
                let window_width = size.width as i32;
                let window_height = size.height as i32;
                let x_offset = if area_width > window_width {
                    (area_width - window_width) / 2
                } else {
                    0
                };
                let y_offset = if area_height > window_height {
                    (area_height - window_height) / 2
                } else {
                    0
                };
                window.set_position(tauri::PhysicalPosition::new(
                    area.position.x + x_offset,
                    area.position.y + y_offset,
                ))
            }
            _ => window.center(),
        };
        let show = window.show();
        // LSUIElement menu-bar apps sometimes need an explicit level bump to raise
        // a hidden preferences window over the previously active app.
        let raise = window.set_always_on_top(true);
        let focus = window.set_focus();
        let lower = window.set_always_on_top(false);
        let after_visible = window.is_visible();
        let after_focused = window.is_focused();
        let after_pos = window.outer_position();
        let after_size = window.outer_size();
        let emit = window.emit("beforepaste-show-panel", panel);
        let panel_js = serde_json::to_string(panel).unwrap_or_else(|_| "\"paste\"".to_string());
        let eval = window.eval(format!(
            "window.__beforepasteRequestedPanel = {panel_js}; try {{ window.localStorage.setItem('beforepaste:v8:last-panel', {panel_js}); }} catch (_error) {{}} window.beforepasteShowPanel && window.beforepasteShowPanel({panel_js});"
        ));
        #[cfg(target_os = "macos")]
        desktop_debug(&format!(
            "show_preferences_panel panel={panel} activation={activation:?} before_visible={before_visible:?} before_minimized={before_minimized:?} before_pos={before_pos:?} before_size={before_size:?} unminimize={unminimize:?} cursor={cursor:?} monitor={monitor_label:?} size_for_place={size_for_place:?} place={place:?} show={show:?} raise={raise:?} focus={focus:?} lower={lower:?} after_visible={after_visible:?} after_focused={after_focused:?} after_pos={after_pos:?} after_size={after_size:?} emit={emit:?} eval={eval:?}"
        ));
        #[cfg(not(target_os = "macos"))]
        desktop_debug(&format!(
            "show_preferences_panel panel={panel} before_visible={before_visible:?} before_minimized={before_minimized:?} before_pos={before_pos:?} before_size={before_size:?} unminimize={unminimize:?} cursor={cursor:?} monitor={monitor_label:?} size_for_place={size_for_place:?} place={place:?} show={show:?} raise={raise:?} focus={focus:?} lower={lower:?} after_visible={after_visible:?} after_focused={after_focused:?} after_pos={after_pos:?} after_size={after_size:?} emit={emit:?} eval={eval:?}"
        ));
        return show.is_ok() && focus.is_ok() && emit.is_ok();
    }
    desktop_debug("show_preferences_panel missing main window");
    false
}

fn schedule_preferences_panel(app: tauri::AppHandle, panel: &'static str) {
    thread::spawn(move || {
        for attempt in 1..=8 {
            thread::sleep(Duration::from_millis(150));
            if show_preferences_panel(&app, panel) {
                desktop_debug(&format!(
                    "show_preferences_panel succeeded attempt={attempt} panel={panel}"
                ));
                return;
            }
        }
    });
}

#[allow(dead_code)]
fn schedule_normal_paste_mode(app: tauri::AppHandle, state: Arc<AppState>, protect: bool) {
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(75));
        if let Err(error) = set_normal_paste_mode(&app, state, protect) {
            desktop_debug(&format!("set_normal_paste_mode failed: {error}"));
        }
    });
}

fn schedule_quit(app: tauri::AppHandle) {
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(75));
        app.exit(0);
    });
}

fn install_preferences_close_handler(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let app_for_hide = app.clone();
    let window_for_hide = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let app = app_for_hide.clone();
            let window = window_for_hide.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(20));
                let _ = window.hide();
                #[cfg(target_os = "macos")]
                let _ = app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            });
        }
    });
}

fn tray_icon() -> Image<'static> {
    Image::from_bytes(include_bytes!("../icons/32x32.png"))
        .expect("BeforePaste tray icon should be a valid PNG")
}

fn tray_lang() -> Lang {
    Config::load().lang
}

fn tray_text(lang: Lang, key: &str) -> &'static str {
    if lang == Lang::ZH {
        match key {
            "status_checking" => "BeforePaste · 本地检查中",
            "safe_paste_clipboard" => "安全粘贴当前剪贴板",
            "preferences" => "设置",
            "quit" => "退出 BeforePaste",
            _ => "",
        }
    } else {
        match key {
            "status_checking" => "BeforePaste · Checking locally",
            "safe_paste_clipboard" => "Safe Paste Current Clipboard",
            "preferences" => "Preferences",
            "quit" => "Quit BeforePaste",
            _ => "",
        }
    }
}

fn build_tray(app: &tauri::App, state: &Arc<AppState>) -> tauri::Result<()> {
    let config = Config::load();
    let lang = config.lang;
    let status = MenuItem::with_id(
        app,
        "status",
        tray_text(lang, "status_checking"),
        false,
        None::<&str>,
    )?;
    let stats = MenuItem::with_id(
        app,
        "session_stats",
        tray_stats_label(state, lang),
        false,
        None::<&str>,
    )?;
    let safe_paste = MenuItem::with_id(
        app,
        "safe_paste_clipboard",
        tray_text(lang, "safe_paste_clipboard"),
        true,
        Some(config.force_paste_hotkey.as_str()),
    )?;
    let open = MenuItem::with_id(
        app,
        "open_preferences",
        tray_text(lang, "preferences"),
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", tray_text(lang, "quit"), true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let separator2 = PredefinedMenuItem::separator(app)?;
    let separator3 = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(
        app,
        &[
            &status,
            &stats,
            &separator,
            &safe_paste,
            &separator2,
            &open,
            &separator3,
            &quit,
        ],
    )?;
    if let Ok(mut tray_menu) = state.tray_menu.lock() {
        *tray_menu = Some(TrayMenuState { status, stats });
    }

    TrayIconBuilder::with_id("main")
        .icon(tray_icon())
        .icon_as_template(false)
        .tooltip("BeforePaste")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(
            |app, event: tauri::menu::MenuEvent| match event.id().as_ref() {
                "safe_paste_clipboard" => {
                    let state = app.state::<Arc<AppState>>().inner().clone();
                    handle_force_redact_paste(state);
                }
                "open_preferences" => schedule_preferences_panel(app.clone(), "paste"),
                "quit" => schedule_quit(app.clone()),
                _ => {}
            },
        )
        .build(app)?;
    Ok(())
}

fn start_target_monitor(app: tauri::AppHandle, state: Arc<AppState>) {
    thread::spawn(move || loop {
        let current =
            protected_paste::current_target_reason().or_else(|| paste_test_target_reason(&state));
        let debug_snapshot = protected_paste::current_target_debug_snapshot();
        state
            .vscode_frontmost
            .store(debug_snapshot_is_vscode(&debug_snapshot), Ordering::SeqCst);
        state.terminal_frontmost.store(
            debug_snapshot_is_terminal(&debug_snapshot),
            Ordering::SeqCst,
        );
        state.vscode_editor_frontmost.store(
            debug_snapshot_is_vscode_editor(&debug_snapshot),
            Ordering::SeqCst,
        );
        log_target_monitor_change(&state, current.as_deref(), &debug_snapshot);
        if let Ok(mut target) = state.target.lock() {
            *target = current;
        }
        update_tray_status(&app, &state);
        thread::sleep(Duration::from_millis(200));
    });
}

fn log_target_monitor_change(state: &Arc<AppState>, current: Option<&str>, debug_snapshot: &str) {
    let label = format!("target={} {debug_snapshot}", current.unwrap_or("none"));
    let Ok(mut previous) = state.target_monitor_label.lock() else {
        return;
    };
    if *previous == label {
        return;
    }
    *previous = label.clone();
    desktop_debug(&format!("target_monitor {label}"));
}

fn debug_snapshot_is_vscode(debug_snapshot: &str) -> bool {
    debug_snapshot.contains("bundle=com.microsoft.VSCode")
        || debug_snapshot.contains("bundle=com.microsoft.VSCodeInsiders")
        || debug_snapshot.contains("bundle=com.visualstudio.code.oss")
}

fn debug_snapshot_is_vscode_editor(debug_snapshot: &str) -> bool {
    debug_snapshot_is_vscode(debug_snapshot)
        && debug_snapshot_vscode_surface(debug_snapshot) == Some("editor")
        && debug_snapshot_vscode_terminal_target(debug_snapshot).is_none()
}

fn debug_snapshot_is_ambiguous_vscode_editor(debug_snapshot: &str) -> bool {
    debug_snapshot_is_vscode(debug_snapshot)
        && debug_snapshot_vscode_surface(debug_snapshot) == Some("editor")
        && debug_snapshot_vscode_terminal_target(debug_snapshot).is_some()
}

fn debug_snapshot_vscode_surface(debug_snapshot: &str) -> Option<&str> {
    let (_, value) = debug_snapshot.split_once("vscode_surface=")?;
    value.split_whitespace().next()
}

fn debug_snapshot_vscode_terminal_target(debug_snapshot: &str) -> Option<&str> {
    let (_, value) = debug_snapshot.split_once("vscode_terminal_target=")?;
    let target = value.split_whitespace().next()?.trim();
    if target.is_empty() || target == "none" {
        None
    } else {
        Some(target)
    }
}

fn debug_snapshot_is_terminal(debug_snapshot: &str) -> bool {
    debug_snapshot.contains("bundle=com.mitchellh.ghostty")
        || debug_snapshot.contains("bundle=com.googlecode.iterm2")
        || debug_snapshot.contains("bundle=com.apple.Terminal")
        || debug_snapshot.contains("bundle=net.kovidgoyal.kitty")
        || debug_snapshot.contains("bundle=com.github.wez.wezterm")
        || debug_snapshot.contains("bundle=dev.warp.Warp-Stable")
        || debug_snapshot.contains("bundle=io.alacritty")
        || debug_snapshot.contains("bundle=co.zeit.hyper")
}

fn update_tray_status(app: &tauri::AppHandle, state: &Arc<AppState>) {
    let status = runtime_status(app, state);
    let labels = tray_labels(&status, state);
    {
        let Ok(mut last) = state.tray_labels.lock() else {
            return;
        };
        if *last == labels {
            return;
        }
        *last = labels.clone();
    };

    let items = state
        .tray_menu
        .lock()
        .ok()
        .and_then(|tray_menu| tray_menu.as_ref().cloned());
    let Some(items) = items else {
        return;
    };

    let _ = items.status.set_text(&labels.status);
    let _ = items.stats.set_text(&labels.stats);
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_tooltip(Some(labels.status.as_str()));
    }
}

#[allow(dead_code)]
fn schedule_tray_status_update(app: tauri::AppHandle, state: Arc<AppState>, delay: Duration) {
    thread::spawn(move || {
        thread::sleep(delay);
        update_tray_status(&app, &state);
    });
}

#[allow(dead_code)]
fn tray_labels(status: &RuntimeStatus, state: &Arc<AppState>) -> TrayLabels {
    let lang = tray_lang();
    let safe = safe_paste_status_label(status, lang);
    let status_label = simple_safe_paste_status_label(status, &safe, lang);
    let stats = tray_stats_label(state, lang);
    TrayLabels {
        status: status_label,
        stats,
    }
}

fn simple_safe_paste_status_label(status: &RuntimeStatus, safe: &str, lang: Lang) -> String {
    if !status.beforepaste_enabled {
        return if lang == Lang::ZH {
            "BeforePaste · 关闭".to_string()
        } else {
            "BeforePaste · Off".to_string()
        };
    }
    if (lang == Lang::ZH && safe.contains("缺少权限"))
        || (lang != Lang::ZH && safe.contains("Missing"))
    {
        return if lang == Lang::ZH {
            format!("BeforePaste · {safe}")
        } else {
            format!("BeforePaste · {safe}")
        };
    }
    if !status.force_paste_hotkey_registered {
        return if lang == Lang::ZH {
            "BeforePaste · 快捷键异常".to_string()
        } else {
            "BeforePaste · Shortcut needs attention".to_string()
        };
    }
    if lang == Lang::ZH {
        "BeforePaste · 本地保护就绪".to_string()
    } else {
        "BeforePaste · Local guard ready".to_string()
    }
}

fn tray_stats_label(state: &Arc<AppState>, lang: Lang) -> String {
    let now = now_secs();
    let Ok(mut cache) = state.tray_stats.lock() else {
        return if lang == Lang::ZH {
            "统计暂不可用".to_string()
        } else {
            "Protected: unavailable".to_string()
        };
    };
    if now.saturating_sub(cache.updated_at) < 5 {
        return cache.label.clone();
    }
    let buckets = stats::read_buckets();
    cache.updated_at = now;
    let protected_since_launch = buckets.total.saturating_sub(cache.baseline_total);
    cache.label = if lang == Lang::ZH {
        format!("本次启动保护：{protected_since_launch}")
    } else {
        format!("Protected since launch: {protected_since_launch}")
    };
    cache.label.clone()
}

#[allow(dead_code)]
fn overall_status_label(status: &RuntimeStatus, cmdv: &str, safe: &str, lang: Lang) -> String {
    if !status.beforepaste_enabled {
        return if lang == Lang::ZH {
            "BeforePaste：保护已关闭".to_string()
        } else {
            "BeforePaste: Protection Off".to_string()
        };
    }
    let ready = if lang == Lang::ZH { "就绪" } else { "Ready" };
    if status.platform == "macos" && status.protect_normal_paste && cmdv != ready {
        return if lang == Lang::ZH {
            format!("BeforePaste：{cmdv}")
        } else {
            format!("BeforePaste: {cmdv}")
        };
    }
    let safe_missing_permission = if lang == Lang::ZH {
        safe.contains("缺少权限")
    } else {
        safe.contains("Missing")
    };
    if safe_missing_permission {
        return if lang == Lang::ZH {
            format!("BeforePaste：{safe}")
        } else {
            format!("BeforePaste: {safe}")
        };
    }
    let safe_needs_attention = if lang == Lang::ZH {
        safe.contains("未注册")
    } else {
        safe.contains("not registered")
    };
    if safe_needs_attention {
        return if lang == Lang::ZH {
            "BeforePaste：安全粘贴快捷键异常".to_string()
        } else {
            "BeforePaste: Safe Paste Needs Attention".to_string()
        };
    }
    if status.platform == "macos" && status.protect_normal_paste {
        if status.current_target.is_some() {
            if lang == Lang::ZH {
                "BeforePaste：保护中".to_string()
            } else {
                "BeforePaste: Ready".to_string()
            }
        } else {
            if lang == Lang::ZH {
                "BeforePaste：已就绪，当前不是 AI 目标".to_string()
            } else {
                "BeforePaste: Ready - No AI target".to_string()
            }
        }
    } else {
        if lang == Lang::ZH {
            "BeforePaste：安全粘贴可用".to_string()
        } else {
            "BeforePaste: Safe Paste Ready".to_string()
        }
    }
}

#[allow(dead_code)]
fn cmdv_status_label(status: &RuntimeStatus, lang: Lang) -> String {
    if !status.beforepaste_enabled {
        return if lang == Lang::ZH {
            "未启用"
        } else {
            "Disabled"
        }
        .to_string();
    }
    if status.platform != "macos" {
        return if lang == Lang::ZH {
            "不支持"
        } else {
            "Not supported"
        }
        .to_string();
    }
    if !status.protect_normal_paste {
        return if lang == Lang::ZH { "关闭" } else { "Off" }.to_string();
    }
    let mut missing = Vec::new();
    if !status.permissions.accessibility {
        missing.push(if lang == Lang::ZH {
            "辅助功能"
        } else {
            "Accessibility"
        });
    }
    if !status.permissions.input_monitoring {
        missing.push(if lang == Lang::ZH {
            "输入监控"
        } else {
            "Input Monitoring"
        });
    }
    if !missing.is_empty() {
        return if lang == Lang::ZH {
            format!("缺少权限：{}", missing.join(" + "))
        } else {
            format!("Missing {}", missing.join(" + "))
        };
    }
    if !status.normal_paste_event_tap_installed {
        return if status.normal_paste_event_tap_started {
            if lang == Lang::ZH {
                "正在恢复"
            } else {
                "Installing"
            }
            .to_string()
        } else {
            if lang == Lang::ZH {
                "需要重启"
            } else {
                "Restart Required"
            }
            .to_string()
        };
    }
    if lang == Lang::ZH { "就绪" } else { "Ready" }.to_string()
}

fn safe_paste_status_label(status: &RuntimeStatus, lang: Lang) -> String {
    if !status.beforepaste_enabled {
        return if lang == Lang::ZH {
            "未启用"
        } else {
            "Disabled"
        }
        .to_string();
    }
    if status.platform == "macos" && !status.permissions.accessibility {
        return if lang == Lang::ZH {
            "缺少权限：辅助功能"
        } else {
            "Missing Accessibility"
        }
        .to_string();
    }
    let hotkey = display_hotkey(&status.force_paste_hotkey);
    if status.force_paste_hotkey_registered {
        if lang == Lang::ZH {
            format!("{hotkey} 可用")
        } else {
            format!("{hotkey} Ready")
        }
    } else {
        if lang == Lang::ZH {
            format!("{hotkey} 未注册")
        } else {
            format!("{hotkey} not registered")
        }
    }
}

#[allow(dead_code)]
fn format_target_reason(reason: Option<&str>, lang: Lang) -> String {
    let Some(reason) = reason else {
        return if lang == Lang::ZH {
            "当前不是 AI 目标"
        } else {
            "Not AI target"
        }
        .to_string();
    };
    let mut parts = reason.split(':');
    let source = parts.next().unwrap_or_default();
    let kind = parts.next().unwrap_or_default();
    match source {
        "cli" => format!("{} CLI", target_label(kind)),
        "app" => {
            if lang == Lang::ZH {
                format!("{} 应用", target_label(kind))
            } else {
                format!("{} app", target_label(kind))
            }
        }
        "web" => {
            if lang == Lang::ZH {
                format!("{} 网页", target_label(kind))
            } else {
                format!("{} web", target_label(kind))
            }
        }
        "shortcut" => if lang == Lang::ZH {
            "安全粘贴"
        } else {
            "Safe paste"
        }
        .to_string(),
        "test" => {
            if lang == Lang::ZH {
                "BeforePaste 测试框".to_string()
            } else {
                "BeforePaste test box".to_string()
            }
        }
        _ => title_case(reason),
    }
}

#[allow(dead_code)]
fn target_label(kind: &str) -> String {
    targets::catalog()
        .iter()
        .find(|entry| entry.id == kind)
        .map(|entry| entry.label.to_string())
        .unwrap_or_else(|| title_case(kind))
}

#[allow(dead_code)]
fn title_case(value: &str) -> String {
    value
        .split([' ', '_', '-'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn display_hotkey(hotkey: &str) -> String {
    hotkey
        .replace("CmdOrCtrl", "Cmd")
        .replace("CommandOrControl", "Cmd")
        .replace("Command", "Cmd")
        .replace("Control", "Ctrl")
        .replace("Key", "")
        .replace("Digit", "")
}

#[cfg(target_os = "macos")]
fn request_accessibility_trust() -> bool {
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};

    use core_foundation::string::CFString;

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXIsProcessTrustedWithOptions(options: CFDictionaryRef) -> bool;
    }

    let prompt_key = CFString::new("AXTrustedCheckOptionPrompt");
    let prompt_value = CFBoolean::true_value();
    let options =
        CFDictionary::from_CFType_pairs(&[(prompt_key.as_CFType(), prompt_value.as_CFType())]);

    unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) }
}

#[cfg(not(target_os = "macos"))]
fn request_accessibility_trust() -> bool {
    true
}

#[cfg(target_os = "macos")]
fn request_input_monitoring_trust_on_main_thread(app: &tauri::AppHandle) -> Result<bool, String> {
    let (tx, rx) = mpsc::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(request_input_monitoring_trust());
    })
    .map_err(|e| e.to_string())?;
    Ok(rx
        .recv_timeout(Duration::from_millis(1200))
        .unwrap_or(false))
}

#[cfg(target_os = "macos")]
fn request_input_monitoring_trust() -> bool {
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGRequestListenEventAccess() -> bool;
    }
    #[link(name = "IOKit", kind = "framework")]
    unsafe extern "C" {
        fn IOHIDRequestAccess(request_type: i32) -> bool;
    }

    // CGEventTap is the path BeforePaste uses for Cmd+V protection, so request
    // CoreGraphics event-listening access first. Keep the IOHID request as a
    // compatibility nudge for the same Input Monitoring privacy pane.
    let cg_granted = unsafe { CGRequestListenEventAccess() };
    // kIOHIDRequestTypeListenEvent.
    let hid_granted = unsafe { IOHIDRequestAccess(1) };
    let tap_granted = trigger_input_monitoring_event_tap_probe();
    cg_granted || hid_granted || tap_granted
}

#[cfg(target_os = "macos")]
fn trigger_input_monitoring_event_tap_probe() -> bool {
    use core_graphics::event::{
        CGEvent, CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
        CGEventType, CallbackResult,
    };

    // Some macOS builds do not add an app to Input Monitoring after the request
    // API alone. Creating a short-lived listen-only tap exercises the exact TCC
    // path that owns the Input Monitoring list without intercepting keystrokes.
    CGEventTap::new(
        CGEventTapLocation::HID,
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::ListenOnly,
        vec![CGEventType::KeyDown],
        |_proxy, _event_type, _event: &CGEvent| CallbackResult::Keep,
    )
    .is_ok()
}

#[cfg(not(target_os = "macos"))]
fn request_input_monitoring_trust() -> bool {
    true
}

#[cfg(target_os = "macos")]
fn accessibility_trusted() -> bool {
    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXIsProcessTrusted() -> bool;
    }

    unsafe { AXIsProcessTrusted() }
}

#[cfg(not(target_os = "macos"))]
fn accessibility_trusted() -> bool {
    true
}

#[cfg(target_os = "macos")]
fn input_monitoring_trusted() -> bool {
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGPreflightListenEventAccess() -> bool;
    }

    unsafe { CGPreflightListenEventAccess() }
}

#[cfg(not(target_os = "macos"))]
fn input_monitoring_trusted() -> bool {
    true
}

#[cfg(target_os = "macos")]
fn event_posting_trusted() -> bool {
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGPreflightPostEventAccess() -> bool;
    }

    unsafe { CGPreflightPostEventAccess() }
}

#[cfg(not(target_os = "macos"))]
fn event_posting_trusted() -> bool {
    true
}

fn permission_status() -> PermissionStatus {
    PermissionStatus {
        accessibility: accessibility_trusted(),
        input_monitoring: input_monitoring_trusted(),
        event_posting: event_posting_trusted(),
        automation: automation_probe(),
    }
}

fn vscode_bridge_status(app: Option<&tauri::AppHandle>) -> VscodeBridgeStatus {
    let installed = vscode_extension_installed();
    let vsix_path = local_vscode_vsix(app).map(|path| path.display().to_string());
    let install_command = vsix_path
        .as_deref()
        .map(|path| format!("code --install-extension {path} --force"))
        .unwrap_or_else(|| "code --install-extension beforepaste-0.1.0.vsix --force".to_string());
    let message = if installed {
        "BeforePaste VS Code extension is installed.".to_string()
    } else if vsix_path.is_some() {
        "Install the BeforePaste VS Code extension to detect AI CLIs in integrated terminals."
            .to_string()
    } else {
        "BeforePaste VS Code extension is not installed, and the local .vsix package was not found."
            .to_string()
    };
    VscodeBridgeStatus {
        installed,
        install_command,
        vsix_path,
        message,
    }
}

fn vscode_extension_installed() -> bool {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return false;
    };
    [
        home.join(".vscode/extensions"),
        home.join(".vscode-insiders/extensions"),
    ]
    .iter()
    .any(|dir| {
        fs::read_dir(dir)
            .ok()
            .into_iter()
            .flatten()
            .flatten()
            .any(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("beforewire.beforepaste-")
            })
    })
}

fn local_vscode_vsix(app: Option<&tauri::AppHandle>) -> Option<PathBuf> {
    if let Some(resource_dir) = app.and_then(|app| app.path().resource_dir().ok()) {
        if let Some(path) = find_vsix_in_dir(resource_dir.join("vscode-extension")) {
            return Some(path);
        }
    }

    let exe = std::env::current_exe().ok();
    let mut dirs = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd.join("vscode-extension"));
        dirs.push(cwd.join("vscode-extension/dist"));
        dirs.push(cwd.join("../vscode-extension"));
        dirs.push(cwd.join("../vscode-extension/dist"));
        dirs.push(cwd.join("../../vscode-extension"));
        dirs.push(cwd.join("../../vscode-extension/dist"));
    }
    if let Some(exe) = exe.as_ref() {
        for ancestor in exe.ancestors().take(8) {
            dirs.push(ancestor.join("vscode-extension"));
            dirs.push(ancestor.join("vscode-extension/dist"));
        }
    }
    dirs.into_iter().find_map(find_vsix_in_dir)
}

fn find_vsix_in_dir(dir: PathBuf) -> Option<PathBuf> {
    let stable = dir.join("beforepaste-vscode.vsix");
    if stable.exists() {
        return Some(stable);
    }
    let versioned = dir.join("beforepaste-0.1.0.vsix");
    if versioned.exists() {
        return Some(versioned);
    }
    fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            path.extension().and_then(|ext| ext.to_str()) == Some("vsix")
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.starts_with("beforepaste"))
                    .unwrap_or(false)
        })
}

fn find_code_cli() -> Option<PathBuf> {
    [
        PathBuf::from("/usr/local/bin/code"),
        PathBuf::from("/opt/homebrew/bin/code"),
        PathBuf::from("/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code"),
        PathBuf::from("code"),
    ]
    .into_iter()
    .find(|path| {
        if path.is_absolute() {
            path.exists()
        } else {
            Command::new(path)
                .arg("--version")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|status| status.success())
                .unwrap_or(false)
        }
    })
}

fn read_last_protected_paste() -> Option<LastProtectedPaste> {
    let data = fs::read_to_string(config::base_dir().join("protected-paste.log")).ok()?;
    for line in data.lines().rev() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some((timestamp, message)) = line.split_once(' ') else {
            continue;
        };
        if message.starts_with("restore:") {
            continue;
        }
        let updated_at = timestamp.parse().ok()?;
        let result = if message.starts_with("redacting:") {
            "Redacted"
        } else if message.starts_with("passthrough:") {
            "Passed through"
        } else if message.starts_with("start:") {
            "Started"
        } else {
            "Info"
        };
        return Some(LastProtectedPaste {
            updated_at,
            result: result.to_string(),
            message: message.to_string(),
        });
    }
    None
}

#[cfg(target_os = "macos")]
fn automation_probe() -> bool {
    let Ok(mut child) = Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg("tell application \"System Events\" to get name of first application process whose frontmost is true")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    else {
        return false;
    };
    let deadline = Instant::now() + Duration::from_millis(800);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(25));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                desktop_debug("automation_probe timed out");
                return false;
            }
            Err(_) => return false,
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn automation_probe() -> bool {
    true
}

fn paste_test_target_reason(state: &Arc<AppState>) -> Option<String> {
    if !state.paste_test_target_active.load(Ordering::SeqCst) {
        return None;
    }
    if beforepaste_is_frontmost() {
        Some("test:beforepaste".to_string())
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
fn beforepaste_is_frontmost() -> bool {
    let Ok(output) = Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(
            "tell application \"System Events\" to get bundle identifier of first application process whose frontmost is true",
        )
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    String::from_utf8_lossy(&output.stdout).trim() == "com.beforewire.beforepaste"
}

#[cfg(not(target_os = "macos"))]
fn beforepaste_is_frontmost() -> bool {
    false
}

#[cfg(target_os = "macos")]
fn start_paste_event_tap(state: Arc<AppState>) -> anyhow::Result<()> {
    use core_foundation::runloop::CFRunLoop;
    use core_graphics::event::{
        CGEvent, CGEventFlags, CGEventTap, CGEventTapLocation, CGEventTapOptions,
        CGEventTapPlacement, CGEventType, CallbackResult, EventField, KeyCode,
    };

    fn is_command_v(event: &CGEvent) -> bool {
        let keycode = event.get_integer_value_field(EventField::KEYBOARD_EVENT_KEYCODE);
        let flags = event.get_flags();
        keycode == i64::from(KeyCode::ANSI_V)
            && flags.contains(CGEventFlags::CGEventFlagCommand)
            && !flags.contains(CGEventFlags::CGEventFlagControl)
    }

    thread::Builder::new()
        .name("beforepaste-event-tap".to_string())
        .spawn(move || {
            if !request_accessibility_trust() {
                desktop_debug("event_tap waiting for accessibility permission");
                eprintln!(
                    "BeforePaste is waiting for macOS Accessibility permission; protected paste shortcuts will pass through until it is granted."
                );
            }
            if !input_monitoring_trusted() {
                let _ = request_input_monitoring_trust();
                desktop_debug("event_tap requested input monitoring permission");
            }
            desktop_debug("event_tap thread starting");

            let running = Arc::new(AtomicBool::new(false));
            let running_for_tap = Arc::clone(&running);
            let state_for_tap = Arc::clone(&state);
            let state_for_status = Arc::clone(&state);
            let result = CGEventTap::with_enabled(
                CGEventTapLocation::HID,
                CGEventTapPlacement::HeadInsertEventTap,
                CGEventTapOptions::Default,
                vec![CGEventType::KeyDown],
                move |_proxy, event_type, event| {
                    if !matches!(event_type, CGEventType::KeyDown) {
                        return CallbackResult::Keep;
                    }

                    if !is_command_v(event) {
                        return CallbackResult::Keep;
                    }

                    if protected_paste::consume_system_paste_bypass() {
                        desktop_debug("cmd_v pass: system paste bypass");
                        return CallbackResult::Keep;
                    }

                    if !state_for_tap
                        .protect_normal_paste
                        .load(Ordering::SeqCst)
                    {
                        desktop_debug("cmd_v pass: protect_normal_paste=false");
                        return CallbackResult::Keep;
                    }
                    if !state_for_tap.beforepaste_enabled.load(Ordering::SeqCst) {
                        desktop_debug("cmd_v pass: beforepaste_enabled=false");
                        return CallbackResult::Keep;
                    }

                    if running_for_tap
                        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                        .is_err()
                    {
                        desktop_debug("cmd_v drop: protected paste already running");
                        return CallbackResult::Drop;
                    }

                    // Keep the event-tap callback fast. Running target detection here can
                    // invoke AppleScript/AX and cause macOS to disable or bypass the tap.
                    let mut reason = state_for_tap
                        .target
                        .lock()
                        .ok()
                        .and_then(|target| target.clone())
                        .or_else(|| paste_test_target_reason(&state_for_tap));
                    if state_for_tap.vscode_editor_frontmost.load(Ordering::SeqCst) {
                        if protected_paste::pending_clipboard_restore_active() {
                            desktop_debug("cmd_v drop: vscode editor pending restore");
                            reason = None;
                        } else {
                            desktop_debug("cmd_v pass: vscode editor");
                            running_for_tap.store(false, Ordering::SeqCst);
                            return CallbackResult::Keep;
                        }
                    }
                    let probe_vscode = reason.is_none()
                        && state_for_tap.vscode_frontmost.load(Ordering::SeqCst);
                    let probe_terminal = reason.is_none()
                        && state_for_tap.terminal_frontmost.load(Ordering::SeqCst);
                    if reason.is_none() && !probe_vscode && !probe_terminal {
                        desktop_debug("cmd_v pass: no cached target");
                        running_for_tap.store(false, Ordering::SeqCst);
                        return CallbackResult::Keep;
                    }
                    if probe_vscode {
                        desktop_debug("cmd_v drop: vscode live probe");
                    } else if probe_terminal {
                        desktop_debug("cmd_v drop: terminal live probe");
                    } else {
                        desktop_debug(&format!(
                            "cmd_v drop: protected paste target={}",
                            reason.as_deref().unwrap_or("unknown")
                        ));
                    }

                    let state_for_paste = Arc::clone(&state_for_tap);
                    let running_for_paste = Arc::clone(&running_for_tap);
                    thread::spawn(move || {
                        let reason = revalidate_paste_target(reason, &state_for_paste);
                        let result = state_for_paste
                            .engine
                            .lock()
                            .map_err(|_| anyhow::anyhow!("engine cache lock poisoned"))
                            .and_then(|mut engine| engine.paste_with_cached_target(reason));
                        if let Err(error) = result {
                            eprintln!("BeforePaste protected paste failed: {error}");
                        }
                        thread::sleep(Duration::from_millis(80));
                        running_for_paste.store(false, Ordering::SeqCst);
                    });
                    CallbackResult::Drop
                },
                move || {
                    state_for_status
                        .normal_paste_event_tap_installed
                        .store(true, Ordering::SeqCst);
                    desktop_debug("event_tap installed");
                    CFRunLoop::run_current();
                    state_for_status
                        .normal_paste_event_tap_started
                        .store(false, Ordering::SeqCst);
                    state_for_status
                        .normal_paste_event_tap_installed
                        .store(false, Ordering::SeqCst);
                    desktop_debug("event_tap stopped");
                },
            );
            if result.is_err() {
                state
                    .normal_paste_event_tap_started
                    .store(false, Ordering::SeqCst);
                state
                    .normal_paste_event_tap_installed
                    .store(false, Ordering::SeqCst);
                desktop_debug("event_tap install failed");
                eprintln!(
                    "BeforePaste failed to install paste event tap. Check Accessibility permission."
                );
            }
        })?;
    Ok(())
}

fn revalidate_paste_target(cached_reason: Option<String>, state: &Arc<AppState>) -> Option<String> {
    if cached_reason
        .as_deref()
        .is_some_and(|reason| reason.starts_with("test:"))
    {
        return cached_reason;
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(cached_reason_value) = cached_reason.clone() {
            let debug_snapshot = protected_paste::current_target_debug_snapshot();
            if debug_snapshot_is_vscode(&debug_snapshot) {
                if debug_snapshot_is_vscode_editor(&debug_snapshot) {
                    desktop_debug(&format!(
                        "cmd_v revalidate: cached={cached_reason_value} live=none {debug_snapshot}"
                    ));
                    return None;
                }
                if debug_snapshot_is_ambiguous_vscode_editor(&debug_snapshot) {
                    return revalidate_ambiguous_vscode_editor(
                        Some(cached_reason_value),
                        state,
                        debug_snapshot,
                    );
                }
                if let Some(reason) = vscode_ai_view_reason_from_debug(&debug_snapshot) {
                    if reason != cached_reason_value {
                        desktop_debug(&format!(
                            "cmd_v revalidate: recovered {reason} from {debug_snapshot}"
                        ));
                    }
                    return Some(reason);
                }
                if let Some(reason) = vscode_terminal_reason_from_debug(&debug_snapshot) {
                    return Some(reason);
                }
                if debug_snapshot_vscode_surface(&debug_snapshot) == Some("terminal") {
                    return Some(cached_reason_value);
                }
                desktop_debug(&format!(
                    "cmd_v revalidate: cached={cached_reason_value} live=none {debug_snapshot}"
                ));
                return None;
            }

            if debug_snapshot_is_terminal(&debug_snapshot) {
                // Repeated AI-terminal pastes should be fast. The monitor refreshes the
                // cached target continuously; use this bundle check only to avoid leaking
                // a stale terminal target into browsers/editors.
                return Some(cached_reason_value);
            }

            desktop_debug(&format!(
                "cmd_v revalidate: cached={cached_reason_value} live=none {debug_snapshot}"
            ));
            return None;
        }

        if state.vscode_frontmost.load(Ordering::SeqCst) {
            let debug_snapshot = protected_paste::current_target_debug_snapshot();
            if debug_snapshot_is_vscode(&debug_snapshot) {
                if debug_snapshot_is_vscode_editor(&debug_snapshot) {
                    return None;
                }
                if debug_snapshot_is_ambiguous_vscode_editor(&debug_snapshot) {
                    return revalidate_ambiguous_vscode_editor(None, state, debug_snapshot);
                }
                if let Some(reason) = vscode_ai_view_reason_from_debug(&debug_snapshot)
                    .or_else(|| vscode_terminal_reason_from_debug(&debug_snapshot))
                {
                    desktop_debug(&format!(
                        "cmd_v revalidate: recovered {reason} from {debug_snapshot}"
                    ));
                    return Some(reason);
                }
                desktop_debug(&format!("cmd_v revalidate: live=none {debug_snapshot}"));
                return None;
            }
        }

        let live_reason = protected_paste::current_target_reason();
        if let Some(reason) = live_reason {
            if cached_reason.as_deref() != Some(reason.as_str()) {
                desktop_debug(&format!(
                    "cmd_v revalidate: cached={} live={}",
                    cached_reason.as_deref().unwrap_or("none"),
                    reason
                ));
            }
            return Some(reason);
        }

        let debug_snapshot = protected_paste::current_target_debug_snapshot();
        if debug_snapshot_is_vscode(&debug_snapshot) {
            if debug_snapshot_is_vscode_editor(&debug_snapshot) {
                if let Some(cached_reason) = cached_reason.as_deref() {
                    desktop_debug(&format!(
                        "cmd_v revalidate: cached={cached_reason} live=none {debug_snapshot}"
                    ));
                }
                return None;
            }
            if debug_snapshot_is_ambiguous_vscode_editor(&debug_snapshot) {
                return revalidate_ambiguous_vscode_editor(cached_reason, state, debug_snapshot);
            }
            if let Some(reason) = vscode_ai_view_reason_from_debug(&debug_snapshot) {
                desktop_debug(&format!(
                    "cmd_v revalidate: recovered {reason} from {debug_snapshot}"
                ));
                return Some(reason);
            }
            if let Some(reason) = vscode_terminal_reason_from_debug(&debug_snapshot) {
                desktop_debug(&format!(
                    "cmd_v revalidate: recovered {reason} from {debug_snapshot}"
                ));
                return Some(reason);
            }
            desktop_debug(&format!("cmd_v revalidate: live=none {debug_snapshot}"));
            return None;
        }

        if let Some(cached_reason) = cached_reason {
            desktop_debug(&format!(
                "cmd_v revalidate: cached={cached_reason} live=none {debug_snapshot}"
            ));
        }
        None
    }

    #[cfg(not(target_os = "macos"))]
    {
        cached_reason
    }
}

#[cfg(target_os = "macos")]
fn revalidate_ambiguous_vscode_editor(
    cached_reason: Option<String>,
    _state: &Arc<AppState>,
    initial_debug_snapshot: String,
) -> Option<String> {
    const RETRY_DELAYS_MS: [u64; 3] = [0, 60, 120];

    for delay_ms in RETRY_DELAYS_MS {
        if delay_ms > 0 {
            thread::sleep(Duration::from_millis(delay_ms));
        }

        if let Some(reason) = protected_paste::current_detected_target_reason() {
            desktop_debug(&format!(
                "cmd_v revalidate: recovered {reason} from ambiguous vscode editor initial={initial_debug_snapshot}"
            ));
            return Some(reason);
        }

        let debug_snapshot = protected_paste::current_target_debug_snapshot();
        if let Some(reason) = vscode_ai_view_reason_from_debug(&debug_snapshot)
            .or_else(|| vscode_terminal_reason_from_debug(&debug_snapshot))
        {
            desktop_debug(&format!(
                "cmd_v revalidate: recovered {reason} from {debug_snapshot}"
            ));
            return Some(reason);
        }

        if debug_snapshot_is_vscode_editor(&debug_snapshot) {
            continue;
        }
        if !debug_snapshot_is_ambiguous_vscode_editor(&debug_snapshot) {
            break;
        }
    }

    if let Some(cached_reason) = cached_reason.as_deref() {
        desktop_debug(&format!(
            "cmd_v revalidate: cached={cached_reason} live=none ambiguous {initial_debug_snapshot}"
        ));
    } else {
        desktop_debug(&format!(
            "cmd_v revalidate: live=none ambiguous {initial_debug_snapshot}"
        ));
    }
    None
}

fn vscode_ai_view_reason_from_debug(debug_snapshot: &str) -> Option<String> {
    let (_, value) = debug_snapshot.split_once("vscode_surface=ai-view:")?;
    let kind = value.split_whitespace().next()?.trim();
    if kind.is_empty() {
        None
    } else {
        Some(format!("cli:{kind}"))
    }
}

fn vscode_terminal_reason_from_debug(debug_snapshot: &str) -> Option<String> {
    if !debug_snapshot_is_vscode(debug_snapshot)
        || debug_snapshot_vscode_surface(debug_snapshot) != Some("terminal")
    {
        return None;
    }
    debug_snapshot_vscode_terminal_target(debug_snapshot).map(|kind| format!("cli:{kind}"))
}

#[cfg(test)]
mod target_snapshot_tests {
    use super::*;

    #[test]
    fn vscode_editor_is_unambiguous_only_without_terminal_target() {
        let editor =
            "bundle=com.microsoft.VSCode vscode_surface=editor vscode_terminal_target=none";
        assert!(debug_snapshot_is_vscode_editor(editor));
        assert!(!debug_snapshot_is_ambiguous_vscode_editor(editor));

        let ambiguous =
            "bundle=com.microsoft.VSCode vscode_surface=editor vscode_terminal_target=codex";
        assert!(!debug_snapshot_is_vscode_editor(ambiguous));
        assert!(debug_snapshot_is_ambiguous_vscode_editor(ambiguous));
    }

    #[test]
    fn recovers_vscode_terminal_reason_only_from_terminal_surface() {
        let terminal =
            "bundle=com.microsoft.VSCode vscode_surface=terminal vscode_terminal_target=codex";
        assert_eq!(
            vscode_terminal_reason_from_debug(terminal).as_deref(),
            Some("cli:codex")
        );

        let editor =
            "bundle=com.microsoft.VSCode vscode_surface=editor vscode_terminal_target=codex";
        assert!(vscode_terminal_reason_from_debug(editor).is_none());
    }
}

#[cfg(not(target_os = "macos"))]
fn start_paste_event_tap(_state: Arc<AppState>) -> anyhow::Result<()> {
    Ok(())
}

fn ensure_normal_paste_event_tap(state: Arc<AppState>) -> anyhow::Result<()> {
    if state
        .normal_paste_event_tap_started
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(());
    }
    if let Err(error) = start_paste_event_tap(Arc::clone(&state)) {
        state
            .normal_paste_event_tap_started
            .store(false, Ordering::SeqCst);
        return Err(error);
    }
    Ok(())
}

fn handle_force_redact_paste(state: Arc<AppState>) {
    thread::spawn(move || {
        let result = state
            .engine
            .lock()
            .map_err(|_| anyhow::anyhow!("engine cache lock poisoned"))
            .and_then(|mut engine| engine.paste_force_redact());
        if result.is_ok() {
            if let Ok(mut cache) = state.tray_stats.lock() {
                cache.updated_at = 0;
            }
        }
        if let Err(error) = result {
            eprintln!("BeforePaste force-redact paste failed: {error}");
        }
    });
}

fn update_force_paste_shortcut(
    app: &tauri::AppHandle,
    state: &Arc<AppState>,
    hotkey: &str,
) -> anyhow::Result<()> {
    let hotkey = hotkey.trim();
    if hotkey.is_empty() {
        anyhow::bail!("force paste shortcut cannot be empty");
    }
    let mut current = state
        .force_paste_hotkey
        .lock()
        .map_err(|_| anyhow::anyhow!("force paste shortcut lock poisoned"))?;
    if current.as_str() == hotkey && app.global_shortcut().is_registered(hotkey) {
        return Ok(());
    }
    app.global_shortcut().register(hotkey)?;
    if !current.is_empty() && current.as_str() != hotkey {
        let _ = app.global_shortcut().unregister(current.as_str());
    }
    *current = hotkey.to_string();
    desktop_debug(&format!("force_paste_hotkey registered={hotkey}"));
    Ok(())
}

#[cfg(target_os = "macos")]
fn sync_launch_at_login(enabled: bool) -> anyhow::Result<()> {
    let plist = launch_agent_path()?;
    let uid = std::process::Command::new("/usr/bin/id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "501".to_string());
    let domain = format!("gui/{uid}");

    if enabled {
        let exe = std::env::current_exe()?;
        let program_args = launch_agent_program_arguments(&exe);
        let plist_body = launch_agent_plist(&program_args);
        if let Some(parent) = plist.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&plist, plist_body)?;
        desktop_debug("launch_at_login enabled");
    } else {
        let _ = Command::new("/bin/launchctl")
            .args(["bootout", &domain])
            .arg(&plist)
            .status();
        let _ = std::fs::remove_file(&plist);
        desktop_debug("launch_at_login disabled");
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn sync_launch_at_login(_enabled: bool) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn launch_agent_path() -> anyhow::Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("HOME is not set"))?;
    Ok(home
        .join("Library")
        .join("LaunchAgents")
        .join("com.beforewire.beforepaste.plist"))
}

#[cfg(target_os = "macos")]
fn launch_agent_program_arguments(exe: &std::path::Path) -> Vec<String> {
    if let Some(app_bundle) = app_bundle_path_from_exe(exe) {
        return vec![
            "/usr/bin/open".to_string(),
            app_bundle.to_string_lossy().into_owned(),
        ];
    }
    vec![exe.to_string_lossy().into_owned()]
}

#[cfg(target_os = "macos")]
fn app_bundle_path_from_exe(exe: &std::path::Path) -> Option<PathBuf> {
    let macos_dir = exe.parent()?;
    if macos_dir.file_name()? != "MacOS" {
        return None;
    }
    let contents_dir = macos_dir.parent()?;
    if contents_dir.file_name()? != "Contents" {
        return None;
    }
    let app_bundle = contents_dir.parent()?;
    if app_bundle.extension()? != "app" {
        return None;
    }
    Some(app_bundle.to_path_buf())
}

#[cfg(target_os = "macos")]
fn launch_agent_plist(program_args: &[String]) -> String {
    let program_args = program_args
        .iter()
        .map(|arg| format!("    <string>{}</string>", escape_plist(arg)))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.beforewire.beforepaste</string>
  <key>ProgramArguments</key>
  <array>
{program_args}
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <false/>
</dict>
</plist>
"#
    )
}

#[cfg(target_os = "macos")]
fn escape_plist(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn main() {
    desktop_debug("main starting");
    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state != ShortcutState::Pressed {
                        return;
                    }
                    let state = app.state::<Arc<AppState>>().inner().clone();
                    handle_force_redact_paste(state);
                })
                .build(),
        )
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            get_config,
            copy_test_payload,
            get_test_payload_status,
            set_paste_test_target,
            get_target_catalog,
            get_cli_target_catalog,
            get_permission_status,
            get_vscode_bridge_status,
            install_vscode_bridge,
            open_privacy_settings,
            reset_macos_permissions,
            get_runtime_status,
            save_config,
            set_manual_target,
            clear_manual_target,
            check_for_update,
            skip_update_version,
            open_url,
            open_logs,
            copy_diagnostic_summary
        ])
        .setup(|app| {
            let mut config = Config::load();
            let should_open_preferences = !config.onboarding_done;
            let mut config_changed = false;
            if config.protect_normal_paste {
                config.protect_normal_paste = false;
                config_changed = true;
            }
            if config.redact_style == config::RedactStyle::Marker {
                config.redact_style = config::RedactStyle::Typed;
                config_changed = true;
            }
            if !config.setup_prompt_dismissed {
                config.setup_prompt_dismissed = true;
                config_changed = true;
            }
            if !config.onboarding_done {
                config.onboarding_done = true;
                config_changed = true;
            }
            if config_changed {
                if let Err(error) = config.save() {
                    eprintln!("BeforePaste failed to persist desktop defaults: {error}");
                }
            }
            let permissions = permission_status();
            desktop_debug(&format!(
                "permissions accessibility={} input_monitoring={} event_posting={} automation={}",
                permissions.accessibility,
                permissions.input_monitoring,
                permissions.event_posting,
                permissions.automation
            ));
            if !permissions.accessibility {
                eprintln!(
                    "BeforePaste is waiting for macOS Accessibility permission; Safe Paste may not paste until it is granted."
                );
            }
            let state = Arc::new(AppState {
                engine: Arc::new(Mutex::new(protected_paste::Engine::from_config(
                    config.clone(),
                ))),
                target: Arc::new(Mutex::new(protected_paste::current_target_reason())),
                force_paste_hotkey: Mutex::new(String::new()),
                beforepaste_enabled: AtomicBool::new(config.beforepaste_enabled),
                protect_normal_paste: AtomicBool::new(config.protect_normal_paste),
                normal_paste_event_tap_started: AtomicBool::new(false),
                normal_paste_event_tap_installed: AtomicBool::new(false),
                terminal_frontmost: AtomicBool::new(false),
                vscode_frontmost: AtomicBool::new(false),
                vscode_editor_frontmost: AtomicBool::new(false),
                tray_menu: Mutex::new(None),
                tray_labels: Mutex::new(TrayLabels::default()),
                tray_stats: Mutex::new(TrayStatsCache::default()),
                target_monitor_label: Mutex::new(String::new()),
                paste_test_target_active: AtomicBool::new(false),
            });
            app.manage(Arc::clone(&state));
            install_preferences_close_handler(&app.handle().clone());
            if let Err(error) = update_force_paste_shortcut(
                &app.handle().clone(),
                &state,
                &config.force_paste_hotkey,
            ) {
                eprintln!("BeforePaste failed to register force paste shortcut: {error}");
            }
            if let Err(error) = sync_launch_at_login(config.launch_at_login) {
                eprintln!("BeforePaste failed to sync launch at login: {error}");
            }
            if config.protect_normal_paste {
                ensure_normal_paste_event_tap(Arc::clone(&state)).map_err(|error| {
                    eprintln!("BeforePaste failed to start paste event tap: {error}");
                    error
                })?;
            }
            desktop_debug("building tray");
            build_tray(app, &state).map_err(|error| {
                desktop_debug(&format!("build_tray failed: {error}"));
                error
            })?;
            desktop_debug("tray built");
            update_tray_status(&app.handle().clone(), &state);
            start_target_monitor(app.handle().clone(), Arc::clone(&state));
            start_update_check(app.handle().clone());
            if should_open_preferences {
                schedule_preferences_panel(app.handle().clone(), "paste");
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running BeforePaste desktop");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime_status(permissions: PermissionStatus, protect_normal_paste: bool) -> RuntimeStatus {
        RuntimeStatus {
            platform: "macos".to_string(),
            permissions,
            beforepaste_enabled: true,
            protect_normal_paste,
            normal_paste_event_tap_started: false,
            normal_paste_event_tap_installed: false,
            force_paste_hotkey: "CommandOrControl+Option+V".to_string(),
            force_paste_hotkey_registered: true,
            current_target: None,
            last_protected_paste: None,
        }
    }

    #[test]
    fn tray_safe_paste_requires_accessibility() {
        let status = runtime_status(
            PermissionStatus {
                accessibility: false,
                input_monitoring: false,
                event_posting: false,
                automation: false,
            },
            false,
        );

        let safe = safe_paste_status_label(&status, Lang::ZH);
        assert_eq!(safe, "缺少权限：辅助功能");
        assert_eq!(
            overall_status_label(&status, "关闭", &safe, Lang::ZH),
            "BeforePaste：缺少权限：辅助功能"
        );
    }

    #[test]
    fn tray_cmdv_reports_missing_input_permissions_first() {
        let status = runtime_status(
            PermissionStatus {
                accessibility: false,
                input_monitoring: false,
                event_posting: false,
                automation: false,
            },
            true,
        );

        let cmdv = cmdv_status_label(&status, Lang::ZH);
        let safe = safe_paste_status_label(&status, Lang::ZH);
        assert_eq!(cmdv, "缺少权限：辅助功能 + 输入监控");
        assert_eq!(
            overall_status_label(&status, &cmdv, &safe, Lang::ZH),
            "BeforePaste：缺少权限：辅助功能 + 输入监控"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn app_bundle_path_is_derived_from_macos_executable() {
        let exe = std::path::Path::new(
            "/Applications/BeforePaste.app/Contents/MacOS/beforepaste-desktop",
        );
        assert_eq!(
            app_bundle_path_from_exe(exe),
            Some(PathBuf::from("/Applications/BeforePaste.app"))
        );
        assert_eq!(
            app_bundle_path_from_exe(std::path::Path::new("/usr/bin/true")),
            None
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn launch_agent_opens_app_bundle_instead_of_raw_executable() {
        let exe = std::path::Path::new(
            "/Applications/BeforePaste.app/Contents/MacOS/beforepaste-desktop",
        );
        let args = launch_agent_program_arguments(exe);
        assert_eq!(
            args,
            vec![
                "/usr/bin/open".to_string(),
                "/Applications/BeforePaste.app".to_string()
            ]
        );
        let plist = launch_agent_plist(&args);
        assert!(plist.contains("<string>/usr/bin/open</string>"));
        assert!(plist.contains("<string>/Applications/BeforePaste.app</string>"));
        assert!(!plist.contains("beforepaste-desktop</string>"));
    }
}
