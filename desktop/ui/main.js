const { invoke } = window.__TAURI__.core;
const tauriEvent = window.__TAURI__.event;

const fields = {
  appSubtitle: document.querySelector("#app-subtitle"),
  panelTitle: document.querySelector("#panel-title"),
  panelSubtitle: document.querySelector("#panel-subtitle"),
  readyPill: document.querySelector("#ready-pill"),
  beforepasteEnabled: document.querySelector("#beforepaste-enabled"),
  forcePasteHotkey: document.querySelector("#force-paste-hotkey"),
  shortcutDisplay: document.querySelector("#shortcut-display"),
  recordShortcut: document.querySelector("#record-shortcut"),
  accessibilityStatus: document.querySelector("#accessibility-status"),
  openAccessibility: document.querySelector("#open-accessibility"),
  startVerification: document.querySelector("#start-verification"),
  runAppVerification: document.querySelector("#run-app-verification"),
  verifyRedaction: document.querySelector("#verify-redaction"),
  verifyShortcut: document.querySelector("#verify-shortcut"),
  verifyPasteAction: document.querySelector("#verify-paste-action"),
  verifyClipboardRestore: document.querySelector("#verify-clipboard-restore"),
  healthShortcut: document.querySelector("#health-shortcut"),
  healthAccessibility: document.querySelector("#health-accessibility"),
  healthRedaction: document.querySelector("#health-redaction"),
  redactStyle: document.querySelector("#redact-style"),
  redactPattern: document.querySelector("#redact-pattern"),
  redactStyleControl: document.querySelector("#redact-style-control"),
  previewApiKey: document.querySelector("#preview-api-key"),
  previewOpenaiKey: document.querySelector("#preview-openai-key"),
  language: document.querySelector("#language"),
  launchAtLogin: document.querySelector("#launch-at-login"),
  checkUpdate: document.querySelector("#check-update"),
  openLogs: document.querySelector("#open-logs"),
  copyDiagnostic: document.querySelector("#copy-diagnostic"),
  updateStatusCard: document.querySelector("#update-status-card"),
  updateStatusTitle: document.querySelector("#update-status-title"),
  updateStatusCopy: document.querySelector("#update-status-copy"),
  downloadUpdate: document.querySelector("#download-update"),
  skipUpdate: document.querySelector("#skip-update"),
  remindUpdateLater: document.querySelector("#remind-update-later"),
};

let currentConfig;
let currentStatus;
let currentLang = "EN";
let saveTimer;
let lastUpdateStatus;
const lastPanelKey = "beforepaste:v8:last-panel";
const supportedRedactStyles = new Set(["Typed", "Placeholder", "Drop"]);

const copy = {
  EN: {
    appSubtitle: "Keep secrets out of AI prompts.",
    sidebarNote: "Local redaction. No clipboard history.",
    panels: {
      paste: "Safe Paste",
      redaction: "Local Rules",
      app: "App & Updates",
    },
    panelSubtitles: {
      paste: "Cmd+V stays unchanged. Use Safe Paste when a prompt needs redaction.",
      redaction: "Name the sensitive values BeforePaste rewrites locally.",
      app: "Language, launch behavior, logs, and release checks.",
    },
    ready: "Local guard ready",
    localRules: "Local rules",
    safePasteClipboard: "Safe Paste shortcut",
    safePasteClipboardCopy: "Turn on the redacted paste path. Normal paste keeps the original clipboard.",
    shortcut: "Shortcut",
    shortcutCopy: "Creates a redacted clipboard payload, pastes it, then restores the original.",
    accessibility: "Paste permission",
    accessibilityCopy: "Allows BeforePaste to send the final paste event. If it is missing in macOS Settings, open the pane and add this app bundle.",
    openSettings: "Open System Settings",
    recordShortcut: "Edit Shortcut",
    testVerify: "Safe Paste preflight",
    testVerifyCopy: "Checks detector output, shortcut registration, paste permission, and restore guard. It does not paste into another app.",
    startVerification: "Run Verification",
    runVerification: "Run Verification",
    notRun: "Not run",
    ok: "OK",
    effective: "Effective",
    readyShort: "Ready",
    missing: "Needs setup",
    off: "Off",
    failed: "Failed",
    redaction: "Detector",
    shortcutLabel: "Shortcut",
    pasteAction: "Paste permission",
    clipboardRestore: "Restore guard",
    footerPaste: "BeforePaste keeps no clipboard history. Redaction runs on this Mac.",
    redactionStyle: "Replacement style",
    redactionStyleCopy: "Choose what an AI app sees after Safe Paste rewrites a match.",
    typedLabels: "Typed placeholders",
    maskValues: "Mask",
    remove: "Remove",
    livePreview: "Paste preview",
    livePreviewCopy: "Raw clipboard values become named placeholders before paste.",
    sensitiveTypes: "Local detectors",
    sensitiveTypesCopy: "Built-in rules that run before any paste action.",
    apiTokens: "API keys & tokens",
    cloudCreds: "Cloud credentials",
    envSecrets: "Secret environment lines",
    on: "On",
    language: "Language",
    openAtLogin: "Open at login",
    openAtLoginCopy: "Start the BeforePaste menu bar guard when you sign in.",
    systemHealth: "Readiness",
    systemHealthCopy: "Shortcut, paste permission, and local detector status.",
    maintenance: "Logs & releases",
    maintenanceCopy: "Local diagnostics and manual update checks.",
    checkUpdates: "Check Releases",
    openLogs: "Open Logs",
    copyDiagnostic: "Copy Diagnostic Summary",
    updateReadyTitle: "Ready to check releases",
    updateReadyCopy: "BeforePaste only contacts GitHub Releases when you ask it to check.",
    updateCheckingTitle: "Checking releases",
    updateCheckingCopy: "Looking for the latest BeforePaste build on GitHub Releases.",
    upToDateTitle: "BeforePaste is up to date",
    upToDateCopy: "Current version: {current}.",
    updateAvailableTitle: "New build available",
    updateAvailableCopy: "{current} -> {latest}. Download the latest build from GitHub Releases.",
    updateSkippedTitle: "Version skipped",
    updateSkippedCopy: "{latest} is hidden. The next release will be shown again.",
    updateFailedTitle: "Could not check releases",
    updateFailedCopy: "Open GitHub Releases manually if you want to download the latest build.",
    downloadLatest: "Download latest build",
    skipVersion: "Skip this version",
    remindLater: "Remind me later",
    remindLaterDone: "Update reminder hidden for now.",
    copiedDiagnostic: "Diagnostic summary copied",
    logsOpened: "Logs opened",
    changesSaved: "Changes saved",
    verificationDone: "Verification complete",
    shortcutHint: "Edit the shortcut field, then leave the field to save it.",
    footerApp: "BeforePaste runs locally and stores no clipboard history.",
  },
  ZH: {
    appSubtitle: "让密钥远离 AI prompts。",
    sidebarNote: "本地脱敏。不保存剪贴板历史。",
    panels: {
      paste: "安全粘贴",
      redaction: "本地规则",
      app: "应用与更新",
    },
    panelSubtitles: {
      paste: "Cmd+V 保持原样；需要脱敏时使用安全粘贴。",
      redaction: "命中敏感值后，BeforePaste 在本机改写为可读占位符。",
      app: "语言、启动行为、日志与版本检查。",
    },
    ready: "本地保护就绪",
    localRules: "本地规则",
    safePasteClipboard: "安全粘贴快捷键",
    safePasteClipboardCopy: "开启独立的脱敏粘贴路径。普通粘贴仍使用原始剪贴板。",
    shortcut: "快捷键",
    shortcutCopy: "生成脱敏剪贴板内容，完成粘贴后恢复原文。",
    accessibility: "粘贴权限",
    accessibilityCopy: "允许 BeforePaste 发送最后一步粘贴事件。如系统设置中缺少本应用，请打开面板并添加当前 app。",
    openSettings: "打开系统设置",
    recordShortcut: "编辑快捷键",
    testVerify: "安全粘贴预检",
    testVerifyCopy: "检查脱敏结果、快捷键注册、粘贴权限和恢复保护；不会向其他应用执行真实粘贴。",
    startVerification: "运行验证",
    runVerification: "运行验证",
    notRun: "未运行",
    ok: "正常",
    effective: "当前可用",
    readyShort: "就绪",
    missing: "待设置",
    off: "关闭",
    failed: "失败",
    redaction: "检测器",
    shortcutLabel: "快捷键",
    pasteAction: "粘贴权限",
    clipboardRestore: "恢复保护",
    footerPaste: "BeforePaste 不保存剪贴板历史；脱敏只在本机运行。",
    redactionStyle: "替换样式",
    redactionStyleCopy: "选择安全粘贴命中敏感值后，AI 应用看到的内容。",
    typedLabels: "类型占位符",
    maskValues: "遮罩",
    remove: "移除",
    livePreview: "粘贴预览",
    livePreviewCopy: "原始剪贴板会在粘贴前变成命名占位符。",
    sensitiveTypes: "本地检测器",
    sensitiveTypesCopy: "粘贴动作发生前运行的内置规则。",
    apiTokens: "API keys 与 tokens",
    cloudCreds: "云凭证",
    envSecrets: "环境变量密钥行",
    on: "开启",
    language: "语言",
    openAtLogin: "登录时启动",
    openAtLoginCopy: "登录系统时启动 BeforePaste 菜单栏保护。",
    systemHealth: "就绪状态",
    systemHealthCopy: "快捷键、粘贴权限与本地检测器状态。",
    maintenance: "日志与版本",
    maintenanceCopy: "本地诊断和手动更新检查。",
    checkUpdates: "检查版本",
    openLogs: "打开日志",
    copyDiagnostic: "复制诊断摘要",
    updateReadyTitle: "可检查版本",
    updateReadyCopy: "只有在你点击检查时，BeforePaste 才会访问 GitHub Releases。",
    updateCheckingTitle: "正在检查版本",
    updateCheckingCopy: "正在 GitHub Releases 查询最新 BeforePaste 构建。",
    upToDateTitle: "BeforePaste 已是最新版本",
    upToDateCopy: "当前版本：{current}。",
    updateAvailableTitle: "发现新构建",
    updateAvailableCopy: "{current} -> {latest}。请从 GitHub Releases 下载最新版本。",
    updateSkippedTitle: "已跳过这个版本",
    updateSkippedCopy: "{latest} 已隐藏；下一个版本会再次提醒。",
    updateFailedTitle: "暂时无法检查版本",
    updateFailedCopy: "如需新版，可以手动打开 GitHub Releases。",
    downloadLatest: "下载最新版本",
    skipVersion: "跳过这个版本",
    remindLater: "稍后提醒",
    remindLaterDone: "已暂时隐藏更新提醒。",
    copiedDiagnostic: "已复制诊断摘要",
    logsOpened: "已打开日志",
    changesSaved: "设置已保存",
    verificationDone: "验证完成",
    shortcutHint: "编辑快捷键字段，离开输入框后保存。",
    footerApp: "BeforePaste 在本机运行，不保存剪贴板历史。",
  },
};

function tr(key) {
  return copy[currentLang]?.[key] ?? copy.EN[key] ?? key;
}

function formatCopy(template, values) {
  return String(template || "").replace(/\{(\w+)\}/g, (_, key) => values?.[key] ?? "");
}

function setStatus(message) {
  fields.readyPill.textContent = message;
  window.clearTimeout(setStatus.timer);
  setStatus.timer = window.setTimeout(() => {
    renderReadyPill();
  }, 2400);
}

function renderReadyPill() {
  if (!currentStatus) {
    fields.readyPill.textContent = tr("ready");
    return;
  }
  if (!currentStatus.beforepaste_enabled) {
    fields.readyPill.textContent = tr("off");
    return;
  }
  fields.readyPill.textContent = hasPastePermission(currentStatus) ? tr("ready") : tr("missing");
}

function hasPastePermission(status) {
  if (status.platform !== "macos") return true;
  const permissions = status.permissions || {};
  return Boolean(permissions.accessibility || permissions.event_posting);
}

function pastePermissionLabel(status) {
  if (status.platform !== "macos") return tr("ok");
  return hasPastePermission(status) ? tr("effective") : tr("missing");
}

function setText(selector, value) {
  const element = document.querySelector(selector);
  if (element) element.textContent = value;
}

function setTexts(selector, values) {
  document.querySelectorAll(selector).forEach((element, index) => {
    if (values[index] !== undefined) element.textContent = values[index];
  });
}

function languageFromConfig(config) {
  return String(config?.lang || "").toUpperCase() === "ZH" ? "ZH" : "EN";
}

function formatHotkeyForDisplay(hotkey) {
  return String(hotkey || "")
    .replaceAll("CmdOrCtrl", "Cmd")
    .replaceAll("CommandOrControl", "Cmd")
    .replaceAll("Command", "Cmd")
    .replaceAll("Control", "Ctrl")
    .replaceAll("Option", "Opt")
    .replace(/\bKey([A-Z])\b/g, "$1")
    .replace(/\bDigit([0-9])\b/g, "$1")
    .replaceAll("+", " + ");
}

function normalizeRedactStyle(style) {
  return supportedRedactStyles.has(style) ? style : "Typed";
}

function applyStaticCopy() {
  document.documentElement.lang = currentLang === "ZH" ? "zh-CN" : "en";
  document.title = currentLang === "ZH" ? "BeforePaste 设置" : "BeforePaste Preferences";
  fields.appSubtitle.textContent = tr("appSubtitle");
  setText(".bp-sidebar-note", tr("sidebarNote"));
  for (const navItem of document.querySelectorAll("[data-panel]")) {
    const label = navItem.querySelector("span:last-child");
    if (label) label.textContent = tr("panels")[navItem.dataset.panel];
  }
  const activePanel = document.querySelector(".bp-nav-item.is-active")?.dataset.panel || "paste";
  fields.panelTitle.textContent = tr("panels")[activePanel];
  fields.panelSubtitle.textContent = tr("panelSubtitles")[activePanel];

  setTexts(".bp-setting-row > span > strong", [
    tr("safePasteClipboard"),
    tr("shortcut"),
    tr("accessibility"),
    tr("redactionStyle"),
    tr("apiTokens"),
    tr("cloudCreds"),
    tr("envSecrets"),
    tr("language"),
    tr("openAtLogin"),
  ]);
  setTexts(".bp-setting-row > span > small", [
    tr("safePasteClipboardCopy"),
    tr("shortcutCopy"),
    tr("accessibilityCopy"),
    tr("redactionStyleCopy"),
    undefined,
    undefined,
    undefined,
    "English · 中文",
    tr("openAtLoginCopy"),
  ]);
  setTexts(".bp-card-header strong", [tr("testVerify"), tr("livePreview")]);
  setTexts(".bp-card-header small", [tr("testVerifyCopy"), tr("livePreviewCopy")]);
  setTexts(".bp-list-header strong", [tr("sensitiveTypes"), tr("systemHealth"), tr("maintenance")]);
  setTexts(".bp-list-header small", [tr("sensitiveTypesCopy"), tr("systemHealthCopy"), tr("maintenanceCopy")]);
  setTexts(".bp-check-row > span:nth-child(2)", [tr("redaction"), tr("shortcutLabel"), tr("pasteAction"), tr("clipboardRestore")]);
  for (const pill of document.querySelectorAll(".bp-setting-row-compact .bp-status-pill")) {
    pill.textContent = tr("on");
  }
  fields.openAccessibility.textContent = tr("openSettings");
  fields.recordShortcut.textContent = tr("recordShortcut");
  fields.startVerification.textContent = tr("startVerification");
  fields.runAppVerification.textContent = tr("runVerification");
  fields.checkUpdate.textContent = tr("checkUpdates");
  fields.openLogs.textContent = tr("openLogs");
  fields.copyDiagnostic.textContent = tr("copyDiagnostic");
  fields.downloadUpdate.textContent = tr("downloadLatest");
  fields.skipUpdate.textContent = tr("skipVersion");
  fields.remindUpdateLater.textContent = tr("remindLater");
  fields.updateStatusTitle.textContent = tr("updateReadyTitle");
  fields.updateStatusCopy.textContent = tr("updateReadyCopy");
  setTexts("[data-redact-style]", [tr("typedLabels"), tr("maskValues"), tr("remove")]);
  setTexts(".bp-footer-note", [tr("footerPaste"), tr("footerApp")]);
  renderReadyPill();
  renderUpdateStatus(lastUpdateStatus);
}

function applyRedactStyle(style) {
  const normalized = normalizeRedactStyle(style);
  fields.redactStyle.value = normalized;
  for (const button of fields.redactStyleControl.querySelectorAll("[data-redact-style]")) {
    button.classList.toggle("is-selected", button.dataset.redactStyle === normalized);
  }
  if (normalized === "Placeholder") {
    fields.previewApiKey.textContent = "api_key: ****-********";
    fields.previewOpenaiKey.textContent = "OPENAI_API_KEY=****-****";
  } else if (normalized === "Drop") {
    fields.previewApiKey.textContent = "api_key: ";
    fields.previewOpenaiKey.textContent = "OPENAI_API_KEY=";
  } else {
    fields.previewApiKey.textContent = "api_key: [API_KEY]";
    fields.previewOpenaiKey.textContent = "OPENAI_API_KEY=[OPENAI_API_KEY]";
  }
}

function renderConfig(config, status) {
  currentConfig = config;
  currentStatus = status;
  currentLang = languageFromConfig(config);
  fields.language.value = currentLang;
  fields.beforepasteEnabled.checked = Boolean(config.beforepaste_enabled);
  fields.forcePasteHotkey.value = config.force_paste_hotkey || "";
  fields.shortcutDisplay.textContent = formatHotkeyForDisplay(config.force_paste_hotkey || "");
  fields.launchAtLogin.checked = Boolean(config.launch_at_login);
  applyRedactStyle(normalizeRedactStyle(config.redact_style));
  applyStaticCopy();
  renderRuntimeStatus(status);
}

function setPill(element, label, state = "muted") {
  element.textContent = label;
  element.classList.remove("is-ok", "is-warn", "is-muted");
  element.classList.add(`is-${state}`);
}

function renderRuntimeStatus(status) {
  currentStatus = status;
  const pastePermissionOk = hasPastePermission(status);
  setPill(fields.accessibilityStatus, pastePermissionLabel(status), pastePermissionOk ? "ok" : "warn");
  renderHealth({
    shortcut: Boolean(status.beforepaste_enabled && status.force_paste_hotkey_registered),
    accessibility: pastePermissionOk,
    redaction: true,
  });
  renderReadyPill();
}

function renderHealth({ shortcut, accessibility, redaction }) {
  setPill(fields.healthShortcut, `${tr("shortcutLabel")} ${shortcut ? tr("ok") : tr("missing")}`, shortcut ? "ok" : "warn");
  setPill(fields.healthAccessibility, `${tr("accessibility")} ${accessibility ? tr("ok") : tr("missing")}`, accessibility ? "ok" : "warn");
  setPill(fields.healthRedaction, `${tr("redaction")} ${redaction ? tr("ok") : tr("failed")}`, redaction ? "ok" : "warn");
}

function setCheck(name, ok, label) {
  const row = document.querySelector(`[data-check="${name}"]`);
  const value = row?.querySelector("strong");
  if (!row || !value) return;
  row.classList.remove("is-ok", "is-warn");
  row.classList.add(ok ? "is-ok" : "is-warn");
  value.textContent = label || (ok ? tr("ok") : tr("failed"));
}

async function runVerification() {
  fields.startVerification.disabled = true;
  fields.runAppVerification.disabled = true;
  try {
    const [status, payload] = await Promise.all([
      invoke("get_runtime_status"),
      invoke("get_test_payload_status"),
    ]);
    renderRuntimeStatus(status);
    const source = String(payload.source || "");
    const redacted = String(payload.redacted || "");
    const redactionOk = redacted !== source
      && !redacted.includes("sk-beforepaste-demo")
      && !redacted.includes("beforepasteDemoSecret");
    const shortcutOk = Boolean(status.beforepaste_enabled && status.force_paste_hotkey_registered);
    const pastePermissionOk = hasPastePermission(status);
    const pasteActionOk = Boolean(status.beforepaste_enabled && pastePermissionOk);
    const restoreOk = Boolean(status.beforepaste_enabled && redactionOk);
    setCheck("redaction", redactionOk);
    setCheck("shortcut", shortcutOk, shortcutOk ? formatHotkeyForDisplay(status.force_paste_hotkey) : tr("missing"));
    setCheck("paste-action", pasteActionOk, pasteActionOk ? tr("readyShort") : tr("missing"));
    setCheck("clipboard-restore", restoreOk, restoreOk ? tr("readyShort") : tr("missing"));
    renderHealth({ shortcut: shortcutOk, accessibility: pastePermissionOk, redaction: redactionOk });
    setStatus(tr("verificationDone"));
  } catch (error) {
    setStatus(String(error));
  } finally {
    fields.startVerification.disabled = false;
    fields.runAppVerification.disabled = false;
  }
}

function collectConfig() {
  return {
    ...currentConfig,
    beforepaste_enabled: fields.beforepasteEnabled.checked,
    protect_normal_paste: false,
    force_paste_hotkey: fields.forcePasteHotkey.value || currentConfig.force_paste_hotkey,
    launch_at_login: fields.launchAtLogin.checked,
    lang: fields.language.value,
    redact_style: fields.redactStyle.value,
    redact_pattern: fields.redactPattern.value || "[REDACTED]",
  };
}

async function saveCurrentConfig() {
  const config = collectConfig();
  await invoke("save_config", { config });
  const status = await invoke("get_runtime_status");
  renderConfig(config, status);
  setStatus(tr("changesSaved"));
}

function queueSave() {
  window.clearTimeout(saveTimer);
  saveTimer = window.setTimeout(() => {
    saveCurrentConfig().catch((error) => {
      setStatus(String(error));
      load().catch((loadError) => setStatus(String(loadError)));
    });
  }, 180);
}

function validPanel(panel) {
  return ["paste", "redaction", "app"].includes(panel);
}

function savedPanelName() {
  try {
    const panel = window.localStorage.getItem(lastPanelKey);
    return validPanel(panel) ? panel : "paste";
  } catch {
    return "paste";
  }
}

function activatePanel(panel, { remember = true } = {}) {
  const nextPanel = validPanel(panel) ? panel : (panel === "doctor" ? "app" : "paste");
  for (const navItem of document.querySelectorAll("[data-panel]")) {
    navItem.classList.toggle("is-active", navItem.dataset.panel === nextPanel);
  }
  for (const content of document.querySelectorAll("[data-panel-content]")) {
    content.classList.toggle("is-active", content.dataset.panelContent === nextPanel);
  }
  fields.panelTitle.textContent = tr("panels")[nextPanel];
  fields.panelSubtitle.textContent = tr("panelSubtitles")[nextPanel];
  if (nextPanel === "redaction") fields.readyPill.textContent = tr("localRules");
  else renderReadyPill();
  if (remember) {
    try { window.localStorage.setItem(lastPanelKey, nextPanel); } catch {}
  }
}

window.beforepasteShowPanel = (panel) => {
  activatePanel(String(panel || "paste"));
};

function setUpdateCard(title, detail, state = "muted") {
  fields.updateStatusTitle.textContent = title;
  fields.updateStatusCopy.textContent = detail;
  fields.updateStatusCard.classList.remove("is-ok", "is-warn", "is-muted");
  fields.updateStatusCard.classList.add(`is-${state}`);
}

function renderUpdateStatus(status) {
  lastUpdateStatus = status || null;
  fields.downloadUpdate.hidden = true;
  fields.skipUpdate.hidden = true;
  fields.remindUpdateLater.hidden = true;
  fields.downloadUpdate.dataset.url = "";
  fields.skipUpdate.dataset.version = "";
  if (!status) {
    setUpdateCard(tr("updateReadyTitle"), tr("updateReadyCopy"), "muted");
    return;
  }
  const current = status.current_version || currentConfig?.version || "";
  const latest = status.version || "";
  if (status.available && status.skipped) {
    setUpdateCard(tr("updateSkippedTitle"), formatCopy(tr("updateSkippedCopy"), { current, latest }), "muted");
    return;
  }
  if (status.available) {
    setUpdateCard(tr("updateAvailableTitle"), formatCopy(tr("updateAvailableCopy"), { current, latest }), "warn");
    const url = status.download_url || status.html_url;
    fields.downloadUpdate.hidden = !url;
    fields.downloadUpdate.dataset.url = url || "";
    fields.skipUpdate.hidden = !latest;
    fields.skipUpdate.dataset.version = latest;
    fields.remindUpdateLater.hidden = false;
    return;
  }
  setUpdateCard(tr("upToDateTitle"), formatCopy(tr("upToDateCopy"), { current, latest }), "ok");
}

async function checkLatestVersion() {
  setUpdateCard(tr("updateCheckingTitle"), tr("updateCheckingCopy"), "muted");
  fields.checkUpdate.disabled = true;
  try {
    const status = await invoke("check_for_update");
    renderUpdateStatus(status);
  } catch (error) {
    fields.downloadUpdate.hidden = false;
    fields.downloadUpdate.dataset.url = "https://github.com/beforewire/beforepaste/releases/latest";
    setUpdateCard(tr("updateFailedTitle"), `${tr("updateFailedCopy")} ${String(error || "")}`.trim(), "warn");
  } finally {
    fields.checkUpdate.disabled = false;
  }
}

async function load() {
  const [configRaw, status] = await Promise.all([
    invoke("get_config"),
    invoke("get_runtime_status"),
  ]);
  const config = { ...configRaw, protect_normal_paste: false };
  config.redact_style = normalizeRedactStyle(config.redact_style);
  renderConfig(config, status);
  const requestedPanel = window.__beforepasteRequestedPanel;
  activatePanel(validPanel(requestedPanel) ? requestedPanel : savedPanelName(), { remember: false });
  if (configRaw.protect_normal_paste || configRaw.redact_style !== config.redact_style) {
    currentConfig = config;
    queueSave();
  }
}

for (const item of document.querySelectorAll("[data-panel]")) {
  item.addEventListener("click", () => activatePanel(item.dataset.panel));
}

for (const field of [fields.beforepasteEnabled, fields.forcePasteHotkey, fields.launchAtLogin, fields.language]) {
  field.addEventListener("change", () => {
    if (field === fields.language) {
      currentLang = fields.language.value;
      applyStaticCopy();
      activatePanel(document.querySelector(".bp-nav-item.is-active")?.dataset.panel || "paste", { remember: false });
    }
    if (field === fields.forcePasteHotkey) {
      fields.shortcutDisplay.textContent = formatHotkeyForDisplay(fields.forcePasteHotkey.value);
    }
    queueSave();
  });
}

fields.recordShortcut.addEventListener("click", () => {
  fields.forcePasteHotkey.focus();
  fields.forcePasteHotkey.select();
  setStatus(tr("shortcutHint"));
});

fields.openAccessibility.addEventListener("click", async () => {
  try {
    await invoke("open_privacy_settings", { kind: "accessibility" });
    const status = await invoke("get_runtime_status");
    renderRuntimeStatus(status);
  } catch (error) {
    setStatus(String(error));
  }
});

fields.startVerification.addEventListener("click", runVerification);
fields.runAppVerification.addEventListener("click", runVerification);

for (const button of fields.redactStyleControl.querySelectorAll("[data-redact-style]")) {
  button.addEventListener("click", () => {
    applyRedactStyle(button.dataset.redactStyle);
    queueSave();
  });
}

fields.checkUpdate.addEventListener("click", () => {
  checkLatestVersion().catch((error) => setStatus(String(error)));
});

fields.openLogs.addEventListener("click", async () => {
  try {
    await invoke("open_logs");
    setStatus(tr("logsOpened"));
  } catch (error) {
    setStatus(String(error));
  }
});

fields.copyDiagnostic.addEventListener("click", async () => {
  try {
    await invoke("copy_diagnostic_summary");
    setStatus(tr("copiedDiagnostic"));
  } catch (error) {
    setStatus(String(error));
  }
});

fields.downloadUpdate.addEventListener("click", async () => {
  const url = fields.downloadUpdate.dataset.url || "https://github.com/beforewire/beforepaste/releases/latest";
  try {
    await invoke("open_url", { url });
  } catch (error) {
    setStatus(String(error));
  }
});

fields.skipUpdate.addEventListener("click", async () => {
  const version = fields.skipUpdate.dataset.version || lastUpdateStatus?.version || "";
  if (!version) return;
  fields.skipUpdate.disabled = true;
  try {
    currentConfig = await invoke("skip_update_version", { version });
    renderUpdateStatus({ ...lastUpdateStatus, skipped: true });
  } catch (error) {
    setStatus(String(error));
  } finally {
    fields.skipUpdate.disabled = false;
  }
});

fields.remindUpdateLater.addEventListener("click", () => {
  fields.downloadUpdate.hidden = true;
  fields.skipUpdate.hidden = true;
  fields.remindUpdateLater.hidden = true;
  setUpdateCard(tr("updateReadyTitle"), tr("remindLaterDone"), "muted");
});

if (tauriEvent?.listen) {
  tauriEvent.listen("beforepaste-show-panel", (event) => {
    activatePanel(String(event.payload || "paste"));
  });
  tauriEvent.listen("beforepaste-config-updated", () => {
    load().catch((error) => setStatus(String(error)));
  });
  tauriEvent.listen("beforepaste-update-status", (event) => {
    renderUpdateStatus(event.payload);
  });
}

window.addEventListener("focus", () => {
  invoke("get_runtime_status")
    .then(renderRuntimeStatus)
    .catch((error) => setStatus(String(error)));
});

load().catch((error) => setStatus(String(error)));
