const { invoke } = window.__TAURI__.core;
const tauriEvent = window.__TAURI__.event;

const fields = {
  language: document.querySelector("#language"),
  beforepasteEnabled: document.querySelector("#beforepaste-enabled"),
  modeAdvanced: document.querySelector("#mode-advanced"),
  modeSafeOnly: document.querySelector("#mode-safe-only"),
  normalPasteTitle: document.querySelector("#normal-paste-title"),
  normalPasteCopy: document.querySelector("#normal-paste-copy"),
  protectNormalPaste: document.querySelector("#protect-normal-paste"),
  forcePasteHotkey: document.querySelector("#force-paste-hotkey"),
  launchAtLogin: document.querySelector("#launch-at-login"),
  deepScan: document.querySelector("#deep-scan"),
  entropyScan: document.querySelector("#entropy-scan"),
  sensitivity: document.querySelector("#sensitivity"),
  checkUpdates: document.querySelector("#check-updates"),
  autoInstall: document.querySelector("#auto-install"),
  redactStyle: document.querySelector("#redact-style"),
  redactPattern: document.querySelector("#redact-pattern"),
  targetList: document.querySelector("#target-list"),
  status: document.querySelector("#status"),
  doctorSummaryTitle: document.querySelector("#doctor-summary-title"),
  doctorSummaryCopy: document.querySelector("#doctor-summary-copy"),
  doctorRefresh: document.querySelector("#doctor-refresh"),
  doctorCurrentTarget: document.querySelector("#doctor-current-target"),
  targetCurrent: document.querySelector("#target-current"),
  doctorNormalPasteLabel: document.querySelector("#doctor-normal-paste-label"),
  doctorCmdV: document.querySelector("#doctor-cmdv"),
  doctorForcePaste: document.querySelector("#doctor-force-paste"),
  doctorAccessibility: document.querySelector("#doctor-accessibility"),
  doctorInputMonitoring: document.querySelector("#doctor-input-monitoring"),
  inputMonitoringCopy: document.querySelector("#input-monitoring-copy"),
  doctorAutomation: document.querySelector("#doctor-automation"),
  doctorLastPaste: document.querySelector("#doctor-last-paste"),
  vscodeBridgeStatus: document.querySelector("#vscode-bridge-status"),
  installVscodeBridge: document.querySelector("#install-vscode-bridge"),
};

let currentConfig;
let currentPlatform = "macos";
let saveTimer;
let targetCatalog = [];
let cliTargetCatalog = [];
let currentLang = "EN";
const lastPanelKey = "beforepaste:last-panel";

const copy = {
  EN: {
    panels: {
      paste: "Paste",
      redaction: "Redaction",
      targets: "Whitelist",
      doctor: "Doctor",
      updates: "Updates",
      advanced: "Advanced",
    },
    statusSaved: "Changes saved",
    statusRefreshed: "Status refreshed",
    usingAutoDetection: "Using auto detection",
    noUpdate: "No update available",
    updateAvailable: "Update available",
    installExtension: "Install extension",
    installed: "Installed",
    notInstalled: "Not installed",
    checking: "Checking",
    installFailed: "Install failed",
    installDone: "Extension installed",
    granted: "✅ Granted",
    missing: "❌ Not granted",
    restartAfterPrivacy: "Restart BeforePaste after changing macOS permissions.",
    recommended: "Recommended",
    web: "Web",
    webPending: "Web pending",
    app: "App",
    appPending: "App pending",
    noTarget: "No AI target frontmost",
    safePaste: "Safe paste",
    ready: "Ready",
    disabled: "Disabled",
    notSupported: "Not supported",
    off: "Off",
    grantPermission: "Grant permission",
    retrying: "Retrying",
    needsRestart: "Needs restart",
    notSet: "Not set",
    notRegistered: "not registered",
    protectionReady: "Protection ready",
    protectionReadyCopy: "BeforePaste can protect the current paste modes.",
    protectionOff: "Protection off",
    protectionOffCopy: "Turn on Enable BeforePaste to use protected paste shortcuts.",
    safeShortcutAttention: "Safe shortcut needs attention",
    safeShortcutAttentionCopy: "Choose another shortcut if this one is already used by another app.",
    needsAttention: "needs attention",
    restartCopy: "Restart BeforePaste or turn normal paste protection off and on again.",
    safeShortcutReady: "Safe shortcut ready",
    noAiTargetReady: "Ready - no AI target",
    lastPasteNone: "No paste recorded",
    manualTarget: "Manual target",
    autoDetection: "Auto",
  },
  ZH: {
    panels: {
      paste: "粘贴保护",
      redaction: "脱敏规则",
      targets: "白名单",
      doctor: "诊断",
      updates: "更新",
      advanced: "高级设置",
    },
    statusSaved: "设置已保存",
    statusRefreshed: "状态已刷新",
    usingAutoDetection: "已切回自动识别",
    noUpdate: "当前已是最新版本",
    updateAvailable: "发现新版本",
    installExtension: "安装插件",
    installed: "已安装",
    notInstalled: "未安装",
    checking: "检查中",
    installFailed: "安装失败",
    installDone: "插件已安装",
    granted: "✅ 已授权",
    missing: "❌ 未授权",
    restartAfterPrivacy: "修改 macOS 权限后，请重启 BeforePaste 再确认。",
    recommended: "推荐",
    web: "网页",
    webPending: "网页未适配",
    app: "应用",
    appPending: "应用未适配",
    noTarget: "当前不是 AI 目标",
    safePaste: "安全粘贴",
    ready: "就绪",
    disabled: "未启用",
    notSupported: "不支持",
    off: "关闭",
    grantPermission: "缺少权限",
    retrying: "正在恢复",
    needsRestart: "需要重启",
    notSet: "未设置",
    notRegistered: "未注册",
    protectionReady: "保护已就绪",
    protectionReadyCopy: "当前粘贴模式可以正常工作。",
    protectionOff: "保护已关闭",
    protectionOffCopy: "打开“启用 BeforePaste”后，粘贴保护才会生效。",
    safeShortcutAttention: "安全粘贴快捷键异常",
    safeShortcutAttentionCopy: "这个快捷键可能被其他应用占用，请换一个快捷键。",
    needsAttention: "需要处理",
    restartCopy: "请重启 BeforePaste，或关闭后重新打开 Cmd+V 保护。",
    safeShortcutReady: "安全粘贴可用",
    noAiTargetReady: "已就绪，当前不是 AI 目标",
    lastPasteNone: "暂无粘贴记录",
    manualTarget: "手动指定目标",
    autoDetection: "自动",
  },
};

function tr(key) {
  return copy[currentLang]?.[key] ?? copy.EN[key] ?? key;
}

function setStatus(message) {
  fields.status.textContent = message;
  window.setTimeout(() => {
    if (fields.status.textContent === message) {
      fields.status.textContent = "";
    }
  }, 2500);
}

function setText(selector, value) {
  const element = document.querySelector(selector);
  if (element) element.textContent = value;
}

function setAllText(selector, values) {
  document.querySelectorAll(selector).forEach((element, index) => {
    if (values[index]) element.textContent = values[index];
  });
}

function languageFromConfig(config) {
  const lang = String(config?.lang || "").toUpperCase();
  if (lang === "ZH") return "ZH";
  if (lang === "EN") return "EN";
  const browserLang = String(navigator.language || "").toLowerCase();
  return browserLang.startsWith("zh") ? "ZH" : "EN";
}

function applyStaticCopy() {
  document.documentElement.lang = currentLang === "ZH" ? "zh-CN" : "en";
  document.title = currentLang === "ZH" ? "BeforePaste 设置" : "BeforePaste Preferences";
  setText(".bp-app-title span", currentLang === "ZH" ? "设置" : "Preferences");

  const panelNames = ["paste", "redaction", "targets", "doctor", "updates", "advanced"];
  for (const panel of panelNames) {
    const nav = document.querySelector(`[data-panel="${panel}"] span:last-child`);
    if (nav) nav.textContent = copy[currentLang].panels[panel];
  }
  const activePanel = document.querySelector(".bp-nav-item.is-active")?.dataset.panel || "paste";
  setText("#panel-title", copy[currentLang].panels[activePanel]);
  setText(".bp-doctor-fact:first-child > span", currentLang === "ZH" ? "目标" : "Target");
  setText(".bp-doctor-fact:nth-child(3) > span", currentLang === "ZH" ? "安全粘贴" : "Safe paste");

  const staticTitleSelector = [
    ".bp-list-header > strong",
    ".bp-setting-row > span > strong",
    ".bp-mode-row > span > strong",
    ".bp-doctor-summary > span > strong",
  ].join(", ");
  const staticCopySelector = [
    ".bp-list-header > small",
    ".bp-setting-row > span > small",
    ".bp-mode-row > span > small",
    ".bp-doctor-summary > span > small",
  ].join(", ");

  setAllText(staticTitleSelector, [
    currentLang === "ZH" ? "启用 BeforePaste" : "Enable BeforePaste",
    currentLang === "ZH" ? "粘贴模式" : "Mode",
    currentLang === "ZH" ? "自动保护 Cmd+V" : "Advanced - Protect Cmd+V",
    currentLang === "ZH" ? "只用安全粘贴快捷键" : "Safe Paste Shortcut Only",
    currentLang === "ZH" ? "安全粘贴快捷键" : "Safe paste shortcut",
    currentLang === "ZH" ? "脱敏方式" : "Redaction style",
    currentLang === "ZH" ? "自定义标记" : "Custom marker",
    currentLang === "ZH" ? "检测强度" : "Detection sensitivity",
    currentLang === "ZH" ? "深度扫描" : "Deep scan",
    currentLang === "ZH" ? "熵扫描" : "Entropy scan",
    currentLang === "ZH" ? "保护白名单" : "Protection whitelist",
    currentLang === "ZH" ? "检查更新" : "Check for updates",
    currentLang === "ZH" ? "自动安装" : "Install automatically",
    currentLang === "ZH" ? "正在检查" : "Checking protection",
    currentLang === "ZH" ? "权限状态" : "Permission checks",
    currentLang === "ZH" ? "辅助功能" : "Accessibility",
    currentLang === "ZH" ? "输入监控" : "Input Monitoring",
    currentLang === "ZH" ? "应用识别" : "App detection capability",
    currentLang === "ZH" ? "上次保护记录" : "Last protected paste",
    currentLang === "ZH" ? "语言" : "Language",
    currentLang === "ZH" ? "登录时自动启动" : "Open at login",
    currentLang === "ZH" ? "临时指定 CLI 目标" : "Manual target",
  ]);

  setAllText(staticCopySelector, [
    currentLang === "ZH" ? "开启后，BeforePaste 才会接管快捷键并进行脱敏粘贴。" : "Allow protected paste shortcuts and target-aware clipboard rewriting.",
    currentLang === "ZH" ? "macOS 推荐使用自动保护 Cmd+V。" : "Advanced is the recommended default on macOS.",
    currentLang === "ZH" ? "在 ChatGPT、Claude、Gemini、Codex 等目标中按 Cmd+V 时，先脱敏再粘贴。" : "Normal Cmd+V is protected in detected AI targets.",
    currentLang === "ZH" ? "不接管 Cmd+V；需要脱敏时使用安全粘贴快捷键。" : "Normal Cmd+V stays unchanged. Use the safe paste shortcut for redacted paste.",
    currentLang === "ZH" ? "无论当前应用是什么，都会粘贴一份脱敏后的内容。" : "Always paste a redacted copy. This remains available in every mode.",
    currentLang === "ZH" ? "选择命中内容在粘贴前如何替换。" : "Choose how matched values are rewritten.",
    currentLang === "ZH" ? "脱敏方式为“固定标记”时使用。" : "Text used when redaction style is set to Marker.",
    currentLang === "ZH" ? "越高覆盖越广，也更容易误判。" : "Higher values catch more patterns and may redact more aggressively.",
    currentLang === "ZH" ? "扫描 JSON、配置片段等结构化内容中的隐藏 secret。" : "Scan structured payloads and embedded secret shapes.",
    currentLang === "ZH" ? "识别随机度很高的未知 token，可能带来更多误判。" : "Detect unknown high-entropy tokens. This can increase false positives.",
    currentLang === "ZH" ? "BeforePaste 只会自动保护勾选的应用、网页和终端场景。" : "BeforePaste only protects the checked apps, websites, and terminal contexts.",
    currentLang === "ZH" ? "发现新的 BeforePaste 版本时提醒我。" : "Look for new BeforePaste releases.",
    currentLang === "ZH" ? "更新通过校验后自动安装。" : "Apply updates after they pass signature checks.",
    currentLang === "ZH" ? "检查粘贴模式、快捷键、目标识别和权限状态。" : "Checks the selected paste mode, shortcuts, target detection, and permissions.",
    currentLang === "ZH" ? "修改 macOS 权限后，可能需要重启 BeforePaste 才能准确刷新。" : "Permission changes may require restarting BeforePaste before macOS reports them accurately.",
    currentLang === "ZH" ? "用于执行最后一步粘贴。修改授权后请重启 BeforePaste 再确认。" : "Needed to perform paste actions. Restart BeforePaste after changing this permission.",
    currentLang === "ZH" ? "用于自动保护 Cmd+V。修改授权后请重启 BeforePaste 再确认。" : "Needed for automatic Cmd+V protection. Restart BeforePaste after changing this permission.",
    currentLang === "ZH" ? "用于识别浏览器标签页和终端上下文。修改授权后请重启 BeforePaste 再确认。" : "Needed to read browser tab and terminal context. Restart BeforePaste after changing this permission.",
    currentLang === "ZH" ? "只显示状态摘要，不会展示剪贴板内容。" : "Status summary only. Secret text is never shown here.",
    currentLang === "ZH" ? "默认跟随系统语言，也可以在这里手动切换。" : "Use the system language when available, or choose a language here.",
    currentLang === "ZH" ? "下次登录系统后自动打开 BeforePaste。" : "Start BeforePaste the next time you sign in.",
    currentLang === "ZH" ? "仅用于排查自动识别问题；30 分钟后自动恢复。" : "Temporary override for AI CLIs when auto detection misses. It expires after 30 minutes.",
  ]);

  document.querySelector(".bp-status-pill.is-on").textContent = tr("recommended");
  document.querySelector("#clear-target").textContent = tr("autoDetection");
  document.querySelector("#check-update").textContent = currentLang === "ZH" ? "检查更新" : "Check for updates";
  const styleOptions = fields.redactStyle?.querySelectorAll("option") || [];
  const styleLabels = currentLang === "ZH"
    ? ["固定标记", "类型标签", "示例占位", "直接删除"]
    : ["Marker", "Typed labels", "Sample values", "Remove values"];
  styleOptions.forEach((option, index) => {
    if (styleLabels[index]) option.textContent = styleLabels[index];
  });
  fields.installVscodeBridge.textContent = tr("installExtension");
  fields.doctorRefresh.textContent = currentLang === "ZH" ? "刷新状态" : "Refresh status";
  for (const button of document.querySelectorAll("[data-open-privacy]")) {
    button.textContent = currentLang === "ZH" ? "打开" : "Open";
  }
}

function renderConfig(config, platform = currentPlatform) {
  currentConfig = config;
  currentPlatform = platform || "macos";
  currentLang = languageFromConfig(config);
  fields.language.value = currentLang;
  applyStaticCopy();
  fields.beforepasteEnabled.checked = Boolean(config.beforepaste_enabled);
  applyPlatformCopy(currentPlatform);
  const advancedMode = currentPlatform === "macos" && Boolean(config.protect_normal_paste);
  fields.modeAdvanced.checked = advancedMode;
  fields.modeAdvanced.disabled = currentPlatform !== "macos";
  fields.modeSafeOnly.checked = !advancedMode;
  fields.protectNormalPaste.checked =
    advancedMode;
  fields.protectNormalPaste.disabled = currentPlatform !== "macos";
  fields.forcePasteHotkey.value = config.force_paste_hotkey;
  fields.launchAtLogin.checked = Boolean(config.launch_at_login);
  fields.deepScan.checked = config.enable_deep_scan;
  fields.entropyScan.checked = config.enable_entropy;
  fields.sensitivity.value = String(config.sensitivity);
  fields.checkUpdates.checked = config.check_for_updates;
  fields.autoInstall.checked = config.auto_install;
  fields.redactStyle.value = config.redact_style;
  fields.redactPattern.value = config.redact_pattern;
  renderTargets();
}

function validPanel(panel) {
  return Boolean(
    panel
    && document.querySelector(`[data-panel="${panel}"]`)
    && document.querySelector(`[data-panel-content="${panel}"]`),
  );
}

function savedPanelName() {
  try {
    const panel = window.localStorage.getItem(lastPanelKey);
    return validPanel(panel) ? panel : "paste";
  } catch {
    return "paste";
  }
}

function rememberPanel(panel) {
  try {
    window.localStorage.setItem(lastPanelKey, panel);
  } catch {
    // localStorage can be unavailable in restricted webviews.
  }
}

function activatePanel(panel, options = {}) {
  const nextPanel = validPanel(panel) ? panel : "paste";
  for (const navItem of document.querySelectorAll("[data-panel]")) {
    navItem.classList.toggle("is-active", navItem.dataset.panel === nextPanel);
  }
  for (const content of document.querySelectorAll("[data-panel-content]")) {
    content.classList.toggle("is-active", content.dataset.panelContent === nextPanel);
  }
  const active = document.querySelector(`[data-panel="${nextPanel}"]`);
  if (active) {
    document.querySelector("#panel-title").textContent =
      copy[currentLang]?.panels?.[nextPanel] ?? active.textContent.trim();
  }
  if (options.remember !== false) {
    rememberPanel(nextPanel);
  }
  if (nextPanel === "doctor") {
    refreshDoctor().catch((error) => {
      setStatus(String(error));
    });
  }
}

function setDiagnosticStatus(element, label, state = "muted") {
  element.textContent = label;
  element.classList.remove("is-ok", "is-warn", "is-muted");
  element.classList.add(`is-${state}`);
}

function formatHotkeyForDisplay(hotkey) {
  return String(hotkey || "")
    .replaceAll("CmdOrCtrl", "Cmd")
    .replaceAll("CommandOrControl", "Cmd")
    .replaceAll("Command", "Cmd")
    .replaceAll("Control", "Ctrl")
    .replace(/\bKey([A-Z])\b/g, "$1")
    .replace(/\bDigit([0-9])\b/g, "$1");
}

function normalPasteLabel(platform = currentPlatform) {
  return platform === "macos" ? "Cmd+V" : "Ctrl+V";
}

function applyPlatformCopy(platform) {
  const label = normalPasteLabel(platform);
  fields.normalPasteTitle.textContent = currentLang === "ZH"
    ? `自动保护 ${label}`
    : `Protect ${label} in AI targets`;
  fields.doctorNormalPasteLabel.textContent = label;
  if (platform === "macos") {
    fields.normalPasteCopy.textContent = currentLang === "ZH"
      ? `在 ChatGPT、Claude、Gemini、Codex 等目标中按 ${label} 时，先脱敏再粘贴。`
      : "Redact before normal paste when an enabled AI app, site, or terminal is frontmost.";
    fields.inputMonitoringCopy.textContent = currentLang === "ZH"
      ? `用于自动保护 ${label}。修改授权后请重启 BeforePaste 再确认。`
      : `Needed for automatic ${label} protection. Restart BeforePaste after changing this permission.`;
  } else {
    fields.normalPasteCopy.textContent = currentLang === "ZH"
      ? "当前平台暂不支持自动保护普通粘贴，请使用安全粘贴快捷键。"
      : "Target-aware normal paste protection is not available on this platform yet. Use the safe paste shortcut.";
    fields.inputMonitoringCopy.textContent = currentLang === "ZH"
      ? "此平台的安全粘贴快捷键不需要输入监控。"
      : "Not required for the safe paste shortcut on this platform.";
  }
}

function titleCase(value) {
  return String(value || "")
    .split(/[\s_-]+/)
    .filter(Boolean)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function targetLabel(kind) {
  const target = targetCatalog.find((entry) => entry.id === kind);
  return target?.label || titleCase(kind);
}

function formatTargetReason(reason) {
  if (!reason) {
    return tr("noTarget");
  }
  const [source, kind, detail] = String(reason).split(":");
  if (source === "cli") {
    return `${targetLabel(kind)} CLI`;
  }
  if (source === "app") {
    return currentLang === "ZH" ? `${targetLabel(kind)} 应用` : `${targetLabel(kind)} app`;
  }
  if (source === "web") {
    return currentLang === "ZH" ? `${targetLabel(kind)} 网页` : `${targetLabel(kind)} web`;
  }
  if (source === "shortcut") {
    return tr("safePaste");
  }
  return titleCase(detail || kind || reason);
}

function permissionLabel(value) {
  return value ? [tr("granted"), "ok"] : [tr("missing"), "warn"];
}

function renderPermission(element, value) {
  const [label, state] = permissionLabel(value);
  setDiagnosticStatus(element, label, state);
}

function renderDoctor(status) {
  currentPlatform = status.platform || currentPlatform;
  applyPlatformCopy(currentPlatform);
  renderPermission(fields.doctorAccessibility, status.permissions.accessibility);
  renderPermission(fields.doctorInputMonitoring, status.permissions.input_monitoring);
  renderPermission(fields.doctorAutomation, status.permissions.automation);

  setDiagnosticStatus(
    fields.doctorCurrentTarget,
    formatTargetReason(status.current_target),
    status.current_target ? "ok" : "muted",
  );
  if (fields.targetCurrent) {
    fields.targetCurrent.textContent = formatTargetReason(status.current_target);
  }

  let cmdVLabel = tr("ready");
  let cmdVState = "ok";
  if (!status.beforepaste_enabled) {
    cmdVLabel = tr("disabled");
    cmdVState = "muted";
  } else if (currentPlatform !== "macos") {
    cmdVLabel = tr("notSupported");
    cmdVState = "muted";
  } else if (!status.protect_normal_paste) {
    cmdVLabel = tr("off");
    cmdVState = "muted";
  } else if (!status.permissions.accessibility || !status.permissions.input_monitoring) {
    cmdVLabel = tr("grantPermission");
    cmdVState = "warn";
  } else if (!status.normal_paste_event_tap_installed) {
    cmdVLabel = status.normal_paste_event_tap_started ? tr("retrying") : tr("needsRestart");
    cmdVState = "warn";
  }
  setDiagnosticStatus(fields.doctorCmdV, cmdVLabel, cmdVState);

  let forceLabel = formatHotkeyForDisplay(status.force_paste_hotkey) || tr("notSet");
  let forceState = "ok";
  if (!status.beforepaste_enabled) {
    forceLabel = tr("disabled");
    forceState = "muted";
  } else if (!status.force_paste_hotkey_registered) {
    forceLabel = `${forceLabel} ${tr("notRegistered")}`;
    forceState = "warn";
  } else {
    forceLabel = currentLang === "ZH" ? `${forceLabel} 可用` : `${forceLabel} ready`;
  }
  setDiagnosticStatus(fields.doctorForcePaste, forceLabel, forceState);

  let summaryTitle = tr("protectionReady");
  let summaryCopy = tr("protectionReadyCopy");
  let summaryState = "ok";
  if (!status.beforepaste_enabled) {
    summaryTitle = tr("protectionOff");
    summaryCopy = tr("protectionOffCopy");
    summaryState = "muted";
  } else if (forceState === "warn") {
    summaryTitle = tr("safeShortcutAttention");
    summaryCopy = tr("safeShortcutAttentionCopy");
    summaryState = "warn";
  } else if (cmdVState === "warn") {
    summaryTitle = `${normalPasteLabel(currentPlatform)} ${tr("needsAttention")}`;
    const missing = [];
    if (!status.permissions.accessibility) missing.push(currentLang === "ZH" ? "辅助功能" : "Accessibility");
    if (!status.permissions.input_monitoring) missing.push(currentLang === "ZH" ? "输入监控" : "Input Monitoring");
    summaryCopy = missing.length
      ? currentLang === "ZH"
        ? `请在 macOS 隐私设置中开启${missing.join("和")}，然后重新打开 BeforePaste。`
        : `Grant ${missing.join(" and ")} in macOS Privacy settings, then reopen BeforePaste.`
      : tr("restartCopy");
    summaryState = "warn";
  } else if (currentPlatform !== "macos") {
    summaryTitle = tr("safeShortcutReady");
    summaryCopy = currentLang === "ZH"
      ? `使用 ${formatHotkeyForDisplay(status.force_paste_hotkey)} 粘贴脱敏后的内容。普通 ${normalPasteLabel(currentPlatform)} 暂不支持自动保护。`
      : `Use ${formatHotkeyForDisplay(status.force_paste_hotkey)} for redacted paste. Normal ${normalPasteLabel(currentPlatform)} protection is not available yet.`;
  } else if (!status.protect_normal_paste) {
    summaryTitle = tr("safeShortcutReady");
    summaryCopy = currentLang === "ZH"
      ? `${normalPasteLabel(currentPlatform)} 自动保护已关闭。使用 ${formatHotkeyForDisplay(status.force_paste_hotkey)} 粘贴脱敏后的内容。`
      : `${normalPasteLabel(currentPlatform)} protection is off. Use ${formatHotkeyForDisplay(status.force_paste_hotkey)} for redacted paste.`;
  } else if (!status.current_target && status.protect_normal_paste) {
    summaryTitle = tr("noAiTargetReady");
    summaryCopy = currentLang === "ZH"
      ? `在 AI 应用、AI 网站或 AI CLI 成为当前目标前，${normalPasteLabel(currentPlatform)} 会正常粘贴。`
      : `${normalPasteLabel(currentPlatform)} will pass through until an enabled AI app, website, or CLI is frontmost.`;
    summaryState = "muted";
  }
  setDiagnosticStatus(fields.doctorSummaryTitle, summaryTitle, summaryState);
  fields.doctorSummaryCopy.textContent = summaryCopy;

  if (status.last_protected_paste) {
    fields.doctorLastPaste.textContent = `${status.last_protected_paste.result}: ${status.last_protected_paste.message}`;
  } else {
    fields.doctorLastPaste.textContent = tr("lastPasteNone");
  }
}

async function refreshDoctor() {
  const status = await invoke("get_runtime_status");
  renderDoctor(status);
}

function activePanelName() {
  return document.querySelector(".bp-nav-item.is-active")?.dataset.panel || "paste";
}

function refreshDoctorIfVisible() {
  if (activePanelName() !== "doctor") return;
  refreshDoctor().catch((error) => {
    setStatus(String(error));
  });
}

async function refreshVscodeBridge() {
  if (!fields.vscodeBridgeStatus) return;
  setDiagnosticStatus(fields.vscodeBridgeStatus, tr("checking"), "muted");
  try {
    const status = await invoke("get_vscode_bridge_status");
    setDiagnosticStatus(
      fields.vscodeBridgeStatus,
      status.installed ? tr("installed") : tr("notInstalled"),
      status.installed ? "ok" : "warn",
    );
    fields.installVscodeBridge.hidden = Boolean(status.installed);
    fields.installVscodeBridge.title = status.install_command || "";
  } catch (error) {
    setDiagnosticStatus(fields.vscodeBridgeStatus, String(error), "warn");
  }
}

function collectConfig() {
  return {
    ...currentConfig,
    beforepaste_enabled: fields.beforepasteEnabled.checked,
    silent: currentConfig.silent,
    protect_normal_paste: fields.protectNormalPaste.disabled
      ? false
      : fields.modeAdvanced.checked,
    force_paste_hotkey: fields.forcePasteHotkey.value || currentConfig.force_paste_hotkey,
    launch_at_login: fields.launchAtLogin.checked,
    enable_deep_scan: fields.deepScan.checked,
    enable_entropy: fields.entropyScan.checked,
    sensitivity: Number(fields.sensitivity.value),
    check_for_updates: fields.checkUpdates.checked,
    auto_install: fields.autoInstall.checked,
    lang: fields.language.value,
    redact_style: fields.redactStyle.value,
    redact_pattern: fields.redactPattern.value || "[REDACTED]",
    disabled_targets: currentConfig.disabled_targets || [],
    disabled_target_surfaces: currentConfig.disabled_target_surfaces || [],
  };
}

async function load() {
  const [config, catalog, cliCatalog, status] = await Promise.all([
    invoke("get_config"),
    invoke("get_target_catalog"),
    invoke("get_cli_target_catalog"),
    invoke("get_runtime_status"),
  ]);
  targetCatalog = catalog;
  cliTargetCatalog = cliCatalog;
  renderConfig(config, status.platform);
  renderDoctor(status);
  await refreshVscodeBridge();
  activatePanel(savedPanelName(), { remember: false });
}

function renderTargets() {
  fields.targetList.textContent = "";
  const appTargets = targetCatalog.filter((target) =>
    target.app_adapted && (target.macos_bundle_ids || []).length > 0
  );
  const webTargets = targetCatalog.filter((target) =>
    target.web_adapted && (target.web_domains || []).length > 0
  );
  renderWhitelistSection("app", sectionCopy("app"), appTargets);
  renderWhitelistSection("web", sectionCopy("web"), webTargets);
  renderWhitelistSection("terminal", sectionCopy("terminal"), cliTargetCatalog);
  renderWhitelistSection("vscode", sectionCopy("vscode"), cliTargetCatalog);
}

function sectionCopy(surface) {
  const zh = currentLang === "ZH";
  const copy = {
    app: {
      title: zh ? "独立应用" : "Standalone apps",
      detail: zh ? "ChatGPT、Claude、Gemini、豆包等独立客户端。" : "Native clients such as ChatGPT, Claude, Gemini, and Doubao.",
    },
    web: {
      title: zh ? "Web 网页" : "Web pages",
      detail: zh ? "浏览器中的 AI 网站，例如 ChatGPT、Gemini、Claude、豆包等。" : "AI websites opened in supported browsers.",
    },
    terminal: {
      title: zh ? "终端面板" : "Terminal panes",
      detail: zh ? "目前已验证 iTerm2 和 Ghostty；其他终端建议使用“只用安全粘贴快捷键”。" : "Currently verified for iTerm2 and Ghostty. Use Safe Paste Shortcut Only for other terminals.",
    },
    vscode: {
      title: zh ? "VS Code 应用" : "VS Code app",
      detail: zh ? "安装 BeforePaste VS Code 插件后，可识别集成终端里的 Codex、Claude Code、Gemini CLI 等。" : "Requires the BeforePaste VS Code extension to identify AI CLIs in integrated terminals.",
    },
  };
  return copy[surface];
}

function surfaceKey(surface, id) {
  return `${surface}:${id}`;
}

function surfaceEnabled(surface, id) {
  const legacyDisabled = new Set(currentConfig.disabled_targets || []);
  const disabled = new Set(currentConfig.disabled_target_surfaces || []);
  return !legacyDisabled.has(id) && !disabled.has(surfaceKey(surface, id));
}

function setSurfaceEnabled(surface, id, enabled) {
  const disabled = new Set(currentConfig.disabled_target_surfaces || []);
  const key = surfaceKey(surface, id);
  if (enabled) {
    disabled.delete(key);
  } else {
    disabled.add(key);
  }
  currentConfig.disabled_target_surfaces = [...disabled].sort();
  queueSave();
}

function renderWhitelistSection(surface, copy, targets) {
  if (!targets.length) return;
  const section = document.createElement("section");
  section.className = "bp-target-section";

  const header = document.createElement("div");
  header.className = "bp-target-section-header";
  const text = document.createElement("span");
  const title = document.createElement("strong");
  title.textContent = copy.title;
  const detail = document.createElement("small");
  detail.textContent = copy.detail;
  text.append(title, detail);
  header.append(text);

  if (surface === "vscode") {
    const actions = document.createElement("div");
    actions.className = "bp-inline-actions";
    actions.append(fields.vscodeBridgeStatus, fields.installVscodeBridge);
    header.append(actions);
  }

  const rows = document.createElement("div");
  rows.className = "bp-target-section-rows";
  for (const target of targets) {
    const row = document.createElement("label");
    row.className = "bp-target-row";

    const rowCopy = document.createElement("span");
    const title = document.createElement("strong");
    title.textContent = target.label;
    const meta = document.createElement("span");
    meta.className = "bp-target-meta";
    meta.textContent = targetMeta(surface, target);
    rowCopy.append(title, meta);

    const switchWrap = document.createElement("span");
    switchWrap.className = "bp-switch";
    const input = document.createElement("input");
    input.type = "checkbox";
    input.checked = surfaceEnabled(surface, target.id);
    input.addEventListener("change", () => {
      setSurfaceEnabled(surface, target.id, input.checked);
    });
    switchWrap.append(input, document.createElement("span"));

    row.append(rowCopy, switchWrap);
    rows.append(row);
  }
  section.append(header, rows);
  fields.targetList.append(section);
}

function targetMeta(surface, target) {
  if (surface === "web") {
    return (target.web_domains || []).join(", ");
  }
  if (surface === "app") {
    return currentLang === "ZH" ? "macOS 独立应用" : "macOS app";
  }
  if (surface === "vscode") {
    return currentLang === "ZH" ? "VS Code 集成终端" : "VS Code integrated terminal";
  }
  return currentLang === "ZH" ? "iTerm2 / Ghostty 终端面板" : "iTerm2 / Ghostty terminal panes";
}

for (const item of document.querySelectorAll("[data-panel]")) {
  item.addEventListener("click", () => {
    activatePanel(item.dataset.panel);
  });
}

async function saveCurrentConfig() {
  const config = collectConfig();
  await invoke("save_config", { config });
  currentConfig = config;
  setStatus(tr("statusSaved"));
}

function queueSave() {
  window.clearTimeout(saveTimer);
  saveTimer = window.setTimeout(() => {
    saveCurrentConfig().catch((error) => {
      setStatus(String(error));
      load().catch((loadError) => setStatus(String(loadError)));
    });
  }, 150);
}

for (const field of [
  fields.language,
  fields.beforepasteEnabled,
  fields.modeAdvanced,
  fields.modeSafeOnly,
  fields.protectNormalPaste,
  fields.launchAtLogin,
  fields.deepScan,
  fields.entropyScan,
  fields.sensitivity,
  fields.checkUpdates,
  fields.autoInstall,
  fields.redactStyle,
]) {
  field.addEventListener("change", queueSave);
}

fields.language.addEventListener("change", () => {
  currentLang = fields.language.value;
  applyStaticCopy();
  renderTargets();
  refreshDoctor().catch((error) => setStatus(String(error)));
  refreshVscodeBridge().catch((error) => setStatus(String(error)));
});

for (const field of [fields.forcePasteHotkey, fields.redactPattern]) {
  field.addEventListener("change", queueSave);
}

for (const button of document.querySelectorAll("[data-target]")) {
  button.addEventListener("click", async () => {
    await invoke("set_manual_target", { kind: button.dataset.target });
    for (const target of document.querySelectorAll("[data-target]")) {
      target.classList.toggle("is-selected", target === button);
    }
    document.querySelector("#clear-target").classList.remove("is-selected");
    setStatus(`${tr("manualTarget")}: ${targetLabel(button.dataset.target)}`);
  });
}

document.querySelector("#clear-target").addEventListener("click", async () => {
  await invoke("clear_manual_target");
  for (const target of document.querySelectorAll("[data-target]")) {
    target.classList.remove("is-selected");
  }
  document.querySelector("#clear-target").classList.add("is-selected");
  setStatus(tr("usingAutoDetection"));
});

document.querySelector("#check-update").addEventListener("click", async () => {
  const result = await invoke("check_for_update");
  if (result.available) {
    setStatus(`${tr("updateAvailable")}: ${result.version}`);
  } else {
    setStatus(tr("noUpdate"));
  }
});

fields.doctorRefresh.addEventListener("click", async () => {
  await refreshDoctor();
  setStatus(tr("statusRefreshed"));
});

fields.installVscodeBridge.addEventListener("click", async () => {
  try {
    await invoke("install_vscode_bridge");
    await refreshVscodeBridge();
    setStatus(tr("installDone"));
  } catch (error) {
    setStatus(`${tr("installFailed")}: ${String(error)}`);
  }
});

for (const button of document.querySelectorAll("[data-open-privacy]")) {
  button.addEventListener("click", async () => {
    try {
      await invoke("open_privacy_settings", { kind: button.dataset.openPrivacy });
      setStatus(tr("restartAfterPrivacy"));
    } catch (error) {
      setStatus(String(error));
    }
  });
}

if (tauriEvent?.listen) {
  tauriEvent.listen("beforepaste-show-panel", (event) => {
    activatePanel(String(event.payload || "paste"));
  });
  tauriEvent.listen("beforepaste-config-updated", () => {
    load().catch((error) => setStatus(String(error)));
  });
}

window.addEventListener("focus", refreshDoctorIfVisible);
document.addEventListener("visibilitychange", () => {
  if (!document.hidden) {
    refreshDoctorIfVisible();
  }
});

load().catch((error) => {
  setStatus(String(error));
});
