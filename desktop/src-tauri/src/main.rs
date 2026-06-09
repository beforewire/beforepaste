#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
#[cfg(target_os = "macos")]
use std::time::Instant;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs, fs::OpenOptions, io::Write};
use std::{path::PathBuf, process::Command};

use beforepaste::config::{self, Config};
use beforepaste::lang::Lang;
use beforepaste::protected_paste;
use beforepaste::stats;
use beforepaste::targets::{self, CliTargetCatalogEntry, TargetCatalogEntry};
use serde::{Deserialize, Serialize};
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::Emitter;
use tauri::Manager;
use tauri::State;
use tauri::WindowEvent;
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tauri_plugin_global_shortcut::ShortcutState;
use tauri_plugin_updater::UpdaterExt;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TargetSnapshot {
    reason: Option<String>,
    updated_at: u64,
    expires_at: u64,
}

#[derive(Debug, Clone, Serialize)]
struct UpdateStatus {
    available: bool,
    version: Option<String>,
    current_version: Option<String>,
    body: Option<String>,
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
struct LastProtectedPaste {
    updated_at: u64,
    result: String,
    message: String,
}

#[derive(Clone)]
struct TrayMenuState {
    status: MenuItem<tauri::Wry>,
    target: MenuItem<tauri::Wry>,
    stats: MenuItem<tauri::Wry>,
    mode_advanced: CheckMenuItem<tauri::Wry>,
    mode_safe_only: CheckMenuItem<tauri::Wry>,
}

#[derive(Clone, Default, PartialEq, Eq)]
struct TrayLabels {
    status: String,
    target: String,
    stats: String,
    advanced_checked: bool,
}

#[derive(Clone)]
struct TrayStatsCache {
    updated_at: u64,
    label: String,
}

impl Default for TrayStatsCache {
    fn default() -> Self {
        Self {
            updated_at: 0,
            label: "Protected 24h: 0".to_string(),
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
    tray_menu: Mutex<Option<TrayMenuState>>,
    tray_labels: Mutex<TrayLabels>,
    tray_stats: Mutex<TrayStatsCache>,
}

#[tauri::command]
fn get_config() -> Config {
    Config::load()
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
        return Err("VS Code 'code' command was not found. Install it from VS Code Command Palette first.".to_string());
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
fn open_privacy_settings(kind: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let url = match kind.as_str() {
            "accessibility" => {
                let _ = request_accessibility_trust();
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
            }
            "input_monitoring" => {
                let _ = request_input_monitoring_trust();
                "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent"
            }
            "automation" => "x-apple.systempreferences:com.apple.preference.security?Privacy_Automation",
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

        let mut errors = Vec::new();
        for service in SERVICES {
            match Command::new("/usr/bin/tccutil")
                .args(["reset", service, BUNDLE_ID])
                .output()
            {
                Ok(output) if output.status.success() => {}
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

        if errors.is_empty() {
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
        .or_else(protected_paste::current_target_reason);

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
        ensure_normal_paste_event_tap(state.inner().clone()).map_err(|e| e.to_string())?;
    }
    let _ = app.emit("beforepaste-config-updated", ());
    Ok(())
}

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
        ensure_normal_paste_event_tap(Arc::clone(&state)).map_err(|e| e.to_string())?;
    }
    schedule_tray_status_update(app.clone(), Arc::clone(&state), Duration::from_millis(150));
    let _ = app.emit("beforepaste-config-updated", ());
    Ok(())
}

#[tauri::command]
fn set_manual_target(kind: String) -> Result<(), String> {
    let kind = match kind.as_str() {
        "codex" | "claude" | "gemini" => kind,
        _ => return Err(format!("unsupported target kind: {kind}")),
    };
    write_target_snapshot(Some(format!("cli:{kind}")), 30 * 60).map_err(|e| e.to_string())
}

#[tauri::command]
fn clear_manual_target() -> Result<(), String> {
    write_target_snapshot(None, 1).map_err(|e| e.to_string())
}

#[tauri::command]
async fn check_for_update(app: tauri::AppHandle) -> Result<UpdateStatus, String> {
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;
    Ok(match update {
        Some(update) => UpdateStatus {
            available: true,
            version: Some(update.version),
            current_version: Some(update.current_version),
            body: update.body,
        },
        None => UpdateStatus {
            available: false,
            version: None,
            current_version: None,
            body: None,
        },
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

fn show_preferences_panel(app: &tauri::AppHandle, panel: &str) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.emit("beforepaste-show-panel", panel);
    }
}

fn schedule_preferences_panel(app: tauri::AppHandle, panel: &'static str) {
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(75));
        show_preferences_panel(&app, panel);
    });
}

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
    let window_for_hide = window.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let window = window_for_hide.clone();
            thread::spawn(move || {
                thread::sleep(Duration::from_millis(20));
                let _ = window.hide();
            });
        }
    });
}

fn tray_icon() -> Image<'static> {
    Image::from_bytes(include_bytes!("../icons/32x32.png")).expect("BeforePaste tray icon should be a valid PNG")
}

fn tray_lang() -> Lang {
    Config::load().lang
}

fn tray_text(lang: Lang, key: &str) -> &'static str {
    if lang == Lang::ZH {
        match key {
            "status_checking" => "状态：检查中",
            "last_target_checking" => "最近目标：检查中",
            "protected_today_zero" => "近 24 小时保护：0",
            "preferences" => "设置",
            "doctor" => "诊断",
            "advanced_mode" => "自动保护 Cmd+V",
            "safe_only_mode" => "只用安全粘贴快捷键",
            "quit" => "退出",
            "mode" => "粘贴模式",
            _ => "",
        }
    } else {
        match key {
            "status_checking" => "Status: Checking",
            "last_target_checking" => "Last target: Checking",
            "protected_today_zero" => "Protected 24h: 0",
            "preferences" => "Preferences",
            "doctor" => "Doctor",
            "advanced_mode" => "Advanced - Protect Cmd+V",
            "safe_only_mode" => "Safe Paste Shortcut Only",
            "quit" => "Quit",
            "mode" => "Mode",
            _ => "",
        }
    }
}

fn build_tray(app: &tauri::App, state: &Arc<AppState>) -> tauri::Result<()> {
    let lang = Config::load().lang;
    let status = MenuItem::with_id(app, "status", tray_text(lang, "status_checking"), true, None::<&str>)?;
    let target = MenuItem::with_id(
        app,
        "target",
        tray_text(lang, "last_target_checking"),
        false,
        None::<&str>,
    )?;
    let stats = MenuItem::with_id(
        app,
        "stats",
        tray_text(lang, "protected_today_zero"),
        false,
        None::<&str>,
    )?;
    let open = MenuItem::with_id(
        app,
        "open_preferences",
        tray_text(lang, "preferences"),
        true,
        None::<&str>,
    )?;
    let doctor = MenuItem::with_id(app, "open_doctor", tray_text(lang, "doctor"), true, None::<&str>)?;
    let advanced = CheckMenuItem::with_id(
        app,
        "mode_advanced",
        tray_text(lang, "advanced_mode"),
        true,
        false,
        None::<&str>,
    )?;
    let safe_only = CheckMenuItem::with_id(
        app,
        "mode_safe_only",
        tray_text(lang, "safe_only_mode"),
        true,
        false,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", tray_text(lang, "quit"), true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let separator2 = PredefinedMenuItem::separator(app)?;
    let mode_menu =
        Submenu::with_id_and_items(app, "mode", tray_text(lang, "mode"), true, &[&advanced, &safe_only])?;
    let menu = Menu::with_items(
        app,
        &[
            &status,
            &target,
            &stats,
            &separator,
            &mode_menu,
            &open,
            &doctor,
            &separator2,
            &quit,
        ],
    )?;
    if let Ok(mut tray_menu) = state.tray_menu.lock() {
        *tray_menu = Some(TrayMenuState {
            status,
            target,
            stats,
            mode_advanced: advanced,
            mode_safe_only: safe_only,
        });
    }

    TrayIconBuilder::with_id("main")
        .icon(tray_icon())
        .icon_as_template(true)
        .tooltip("BeforePaste")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(
            |app, event: tauri::menu::MenuEvent| match event.id().as_ref() {
                "status" => schedule_preferences_panel(app.clone(), "doctor"),
                "open_preferences" => schedule_preferences_panel(app.clone(), "paste"),
                "open_doctor" => schedule_preferences_panel(app.clone(), "doctor"),
                "mode_advanced" => {
                    let state = app.state::<Arc<AppState>>().inner().clone();
                    schedule_normal_paste_mode(app.clone(), state, true);
                }
                "mode_safe_only" => {
                    let state = app.state::<Arc<AppState>>().inner().clone();
                    schedule_normal_paste_mode(app.clone(), state, false);
                }
                "quit" => schedule_quit(app.clone()),
                _ => {}
            },
        )
        .build(app)?;
    Ok(())
}

fn start_target_monitor(app: tauri::AppHandle, state: Arc<AppState>) {
    thread::spawn(move || loop {
        let current = protected_paste::current_target_reason();
        if let Ok(mut target) = state.target.lock() {
            *target = current;
        }
        update_tray_status(&app, &state);
        thread::sleep(Duration::from_millis(400));
    });
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
    let _ = items.target.set_text(&labels.target);
    let _ = items.stats.set_text(&labels.stats);
    let _ = items.mode_advanced.set_checked(labels.advanced_checked);
    let _ = items.mode_safe_only.set_checked(!labels.advanced_checked);
    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_tooltip(Some(labels.status.as_str()));
    }
}

fn schedule_tray_status_update(app: tauri::AppHandle, state: Arc<AppState>, delay: Duration) {
    thread::spawn(move || {
        thread::sleep(delay);
        update_tray_status(&app, &state);
    });
}

fn tray_labels(status: &RuntimeStatus, state: &Arc<AppState>) -> TrayLabels {
    let lang = tray_lang();
    let target = format_target_reason(status.current_target.as_deref(), lang);
    let cmdv = cmdv_status_label(status, lang);
    let safe = safe_paste_status_label(status, lang);
    let status_label = overall_status_label(status, &cmdv, &safe, lang);
    let stats = tray_stats_label(state, lang);
    TrayLabels {
        status: status_label,
        target: if lang == Lang::ZH {
            format!("最近目标：{target}")
        } else {
            format!("Last target: {target}")
        },
        stats,
        advanced_checked: status.platform == "macos" && status.protect_normal_paste,
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
    cache.label = if lang == Lang::ZH {
        format!("近 24 小时保护：{}", buckets.last_24h)
    } else {
        format!("Protected 24h: {}", buckets.last_24h)
    };
    cache.label.clone()
}

fn overall_status_label(status: &RuntimeStatus, cmdv: &str, safe: &str, lang: Lang) -> String {
    if !status.beforepaste_enabled {
        return if lang == Lang::ZH {
            "BeforePaste：保护已关闭".to_string()
        } else {
            "BeforePaste: Protection Off".to_string()
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
    let ready = if lang == Lang::ZH { "就绪" } else { "Ready" };
    if status.platform == "macos" && status.protect_normal_paste && cmdv != ready {
        return if lang == Lang::ZH {
            format!("BeforePaste：{cmdv}")
        } else {
            format!("BeforePaste: {cmdv}")
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

fn cmdv_status_label(status: &RuntimeStatus, lang: Lang) -> String {
    if !status.beforepaste_enabled {
        return if lang == Lang::ZH { "未启用" } else { "Disabled" }.to_string();
    }
    if status.platform != "macos" {
        return if lang == Lang::ZH { "不支持" } else { "Not supported" }.to_string();
    }
    if !status.protect_normal_paste {
        return if lang == Lang::ZH { "关闭" } else { "Off" }.to_string();
    }
    let mut missing = Vec::new();
    if !status.permissions.accessibility {
        missing.push(if lang == Lang::ZH { "辅助功能" } else { "Accessibility" });
    }
    if !status.permissions.input_monitoring {
        missing.push(if lang == Lang::ZH { "输入监控" } else { "Input Monitoring" });
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
            if lang == Lang::ZH { "正在恢复" } else { "Installing" }.to_string()
        } else {
            if lang == Lang::ZH { "需要重启" } else { "Restart Required" }.to_string()
        };
    }
    if lang == Lang::ZH { "就绪" } else { "Ready" }.to_string()
}

fn safe_paste_status_label(status: &RuntimeStatus, lang: Lang) -> String {
    if !status.beforepaste_enabled {
        return if lang == Lang::ZH { "未启用" } else { "Disabled" }.to_string();
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

fn format_target_reason(reason: Option<&str>, lang: Lang) -> String {
    let Some(reason) = reason else {
        return if lang == Lang::ZH { "当前不是 AI 目标" } else { "Not AI target" }.to_string();
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
        "shortcut" => {
            if lang == Lang::ZH { "安全粘贴" } else { "Safe paste" }.to_string()
        }
        _ => title_case(reason),
    }
}

fn target_label(kind: &str) -> String {
    targets::catalog()
        .iter()
        .find(|entry| entry.id == kind)
        .map(|entry| entry.label.to_string())
        .unwrap_or_else(|| title_case(kind))
}

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
fn request_input_monitoring_trust() -> bool {
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGRequestListenEventAccess() -> bool;
    }

    unsafe { CGRequestListenEventAccess() }
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
        "Install the BeforePaste VS Code extension to detect AI CLIs in integrated terminals.".to_string()
    } else {
        "BeforePaste VS Code extension is not installed, and the local .vsix package was not found.".to_string()
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

                    let reason = state_for_tap
                        .target
                        .lock()
                        .ok()
                        .and_then(|target| target.clone())
                        .or_else(protected_paste::current_target_reason);
                    if reason.is_none() {
                        desktop_debug("cmd_v pass: no target");
                        running_for_tap.store(false, Ordering::SeqCst);
                        return CallbackResult::Keep;
                    }
                    desktop_debug(&format!(
                        "cmd_v drop: protected paste target={}",
                        reason.as_deref().unwrap_or("unknown")
                    ));

                    let state_for_paste = Arc::clone(&state_for_tap);
                    let running_for_paste = Arc::clone(&running_for_tap);
                    thread::spawn(move || {
                        let result = state_for_paste
                            .engine
                            .lock()
                            .map_err(|_| anyhow::anyhow!("engine cache lock poisoned"))
                            .and_then(|mut engine| engine.paste_with_cached_target(reason));
                        if let Err(error) = result {
                            eprintln!("BeforePaste protected paste failed: {error}");
                        }
                        thread::sleep(Duration::from_millis(1000));
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
        let plist_body = launch_agent_plist(&exe.to_string_lossy());
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
fn launch_agent_plist(program: &str) -> String {
    let program = escape_plist(program);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.beforewire.beforepaste</string>
  <key>ProgramArguments</key>
  <array>
    <string>{program}</string>
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
            check_for_update
        ])
        .setup(|app| {
            let config = Config::load();
            let permissions = permission_status();
            desktop_debug(&format!(
                "permissions accessibility={} input_monitoring={} event_posting={} automation={}",
                permissions.accessibility,
                permissions.input_monitoring,
                permissions.event_posting,
                permissions.automation
            ));
            if !request_accessibility_trust() {
                eprintln!(
                    "BeforePaste is waiting for macOS Accessibility permission; protected paste shortcuts may not paste until it is granted."
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
                tray_menu: Mutex::new(None),
                tray_labels: Mutex::new(TrayLabels::default()),
                tray_stats: Mutex::new(TrayStatsCache::default()),
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
            build_tray(app, &state)?;
            update_tray_status(&app.handle().clone(), &state);
            start_target_monitor(app.handle().clone(), Arc::clone(&state));
            if !config.onboarding_done {
                schedule_preferences_panel(app.handle().clone(), "paste");
                let mut next = config.clone();
                next.onboarding_done = true;
                if let Err(error) = next.save() {
                    eprintln!("BeforePaste failed to mark onboarding as shown: {error}");
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running BeforePaste desktop");
}
