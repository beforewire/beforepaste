const { invoke } = window.__TAURI__.core;
const tauriEvent = window.__TAURI__.event;

const fields = {
  language: document.querySelector("#language"),
  setupCard: document.querySelector("#setup-card"),
  setupTitle: document.querySelector("#setup-title"),
  setupCopy: document.querySelector("#setup-copy"),
  setupOpenDoctor: document.querySelector("#setup-open-doctor"),
  setupDismissPrompt: document.querySelector("#setup-dismiss-prompt"),
  setupPermissionsTitle: document.querySelector("#setup-permissions-title"),
  setupPermissionsCopy: document.querySelector("#setup-permissions-copy"),
  setupPermissionsStatus: document.querySelector("#setup-permissions-status"),
  setupOpenPermissions: document.querySelector("#setup-open-permissions"),
  setupCmdvTitle: document.querySelector("#setup-cmdv-title"),
  setupCmdvCopy: document.querySelector("#setup-cmdv-copy"),
  setupCmdvStatus: document.querySelector("#setup-cmdv-status"),
  setupSafeTitle: document.querySelector("#setup-safe-title"),
  setupSafeCopy: document.querySelector("#setup-safe-copy"),
  setupSafeStatus: document.querySelector("#setup-safe-status"),
  setupVscodeTitle: document.querySelector("#setup-vscode-title"),
  setupVscodeCopy: document.querySelector("#setup-vscode-copy"),
  setupVscodeStatus: document.querySelector("#setup-vscode-status"),
  setupInstallVscodeBridge: document.querySelector("#setup-install-vscode-bridge"),
  setupSkipVscodeBridge: document.querySelector("#setup-skip-vscode-bridge"),
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
  updateStatusCard: document.querySelector("#update-status-card"),
  updateStatusTitle: document.querySelector("#update-status-title"),
  updateStatusCopy: document.querySelector("#update-status-copy"),
  checkUpdate: document.querySelector("#check-update"),
  downloadUpdate: document.querySelector("#download-update"),
  skipUpdate: document.querySelector("#skip-update"),
  remindUpdateLater: document.querySelector("#remind-update-later"),
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
  doctorResetPermissions: document.querySelector("#doctor-reset-permissions"),
  doctorLastPaste: document.querySelector("#doctor-last-paste"),
  doctorVscodeTitle: document.querySelector("#doctor-vscode-title"),
  doctorVscodeCopy: document.querySelector("#doctor-vscode-copy"),
  doctorVscodeBridge: document.querySelector("#doctor-vscode-bridge"),
  doctorInstallVscodeBridge: document.querySelector("#doctor-install-vscode-bridge"),
  pasteTestTitle: document.querySelector("#paste-test-title"),
  pasteTestCopy: document.querySelector("#paste-test-copy"),
  pasteTestNote: document.querySelector("#paste-test-note"),
  pasteTestSourceLabel: document.querySelector("#paste-test-source-label"),
  pasteTestSource: document.querySelector("#paste-test-source"),
  pasteTestOutputLabel: document.querySelector("#paste-test-output-label"),
  pasteTestOutput: document.querySelector("#paste-test-output"),
  pasteTestResult: document.querySelector("#paste-test-result"),
  pasteTestResultTitle: document.querySelector("#paste-test-result-title"),
  pasteTestResultCopy: document.querySelector("#paste-test-result-copy"),
  copyTestPayload: document.querySelector("#copy-test-payload"),
  vscodeBridgeStatus: document.querySelector("#vscode-bridge-status"),
  installVscodeBridge: document.querySelector("#install-vscode-bridge"),
};

let currentConfig;
let currentPlatform = "macos";
let saveTimer;
let targetCatalog = [];
let cliTargetCatalog = [];
let currentLang = "EN";
let lastRuntimeStatus;
let lastVscodeBridgeStatus;
let lastTestPayloadStatus;
let lastUpdateStatus;
let pasteTestTargetActive = false;
const pendingPrivacyChecks = new Set();
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
    noUpdate: "Open GitHub Releases to download the latest build.",
    updateAvailable: "Update available",
    installExtension: "Install extension",
    installed: "Installed",
    notInstalled: "Not installed",
    checking: "Checking",
    installFailed: "Install failed",
    installDone: "Extension installed",
    skipVscode: "I don't use VS Code",
    vscodeDismissed: "VS Code reminder hidden. You can still install the extension from Doctor.",
    granted: "✅ Granted",
    missing: "❌ Not granted",
    available: "✅ Available",
    unavailable: "❌ Unavailable",
    openSettings: "Open settings",
    copySample: "Copy sample",
    sampleCopied: "Sample copied. Paste it into the test box with Cmd+V or the Safe Paste shortcut.",
    setupDismissed: "Launch reminder dismissed. You can still open Preferences from the tray.",
    restartPending: "Restart to confirm",
    resettingPermissions: "Resetting macOS permission records...",
    restartAfterPrivacy: "After granting access, quit and reopen BeforePaste. Trust Doctor status over System Settings labels.",
    resetPermissions: "Reset macOS permissions",
    resetPermissionsConfirm: "This clears BeforePaste permission records for Accessibility, Input Monitoring, Paste Events, and Automation. Continue?",
    resetPermissionsDone: "BeforePaste permission records were reset. Quit and reopen BeforePaste, then request access again from Doctor.",
    resetPermissionsFailed: "Permission reset failed",
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
    restartCopy: "Restart BeforePaste. If this started after a preview update, reset macOS permissions and grant access again.",
    safeShortcutReady: "Safe shortcut ready",
    noAiTargetReady: "Ready - no AI target",
    lastPasteNone: "No paste recorded",
    manualTarget: "Manual target",
    autoDetection: "Auto",
    testWaitingTitle: "Waiting for test",
    testWaitingCopy: "Copy the sample, then paste it into the test box.",
    testCopiedTitle: "Sample copied",
    testCopiedCopy: "Click the test box and press Cmd+V. Safe Paste also works here.",
    testSuccessTitle: "Protection works",
    testSuccessCopy: "BeforePaste redacted the sample locally before it reached the test box.",
    testRawTitle: "Still seeing raw text",
    testRawCopy: "Check Input Monitoring for automatic Cmd+V, or try the Safe Paste shortcut.",
    testChangedTitle: "Paste result changed",
    testChangedCopy: "The text does not match the expected protected sample yet.",
    updateCheckingTitle: "Checking for updates",
    updateCheckingCopy: "BeforePaste checks GitHub Releases and only reminds you when a newer build is available.",
    updateReadyTitle: "BeforePaste is up to date",
    updateReadyCopy: "Current version: {current}.",
    updateAvailableTitle: "New version available",
    updateAvailableCopy: "{current} → {latest}. Download the newest preview build from GitHub Releases.",
    updateSkippedTitle: "Version skipped",
    updateSkippedCopy: "{latest} is hidden. The next release will be shown again.",
    updateFailedTitle: "Could not check updates",
    updateFailedCopy: "Open GitHub Releases manually if you want to download the latest build.",
    checkUpdate: "Check for update",
    downloadUpdate: "Download latest build",
    skipUpdate: "Skip this version",
    remindLater: "Remind me later",
    remindLaterDone: "Update reminder hidden for now.",
    skipUpdateDone: "This version will be skipped.",
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
    noUpdate: "请前往 GitHub Releases 下载最新版本。",
    updateAvailable: "发现新版本",
    installExtension: "安装插件",
    installed: "已安装",
    notInstalled: "未安装",
    checking: "检查中",
    installFailed: "安装失败",
    installDone: "插件已安装",
    skipVscode: "我不用 VS Code",
    vscodeDismissed: "已隐藏 VS Code 插件提醒；之后仍可在诊断页安装。",
    granted: "✅ 已授权",
    missing: "❌ 未授权",
    available: "✅ 当前可用",
    unavailable: "❌ 不可用",
    openSettings: "打开设置",
    copySample: "复制测试内容",
    sampleCopied: "测试内容已复制。可以在测试框里直接按 Cmd+V，也可以按安全粘贴快捷键。",
    setupDismissed: "已关闭启动提醒；之后仍可从托盘打开设置。",
    restartPending: "重启后确认",
    resettingPermissions: "正在重置 macOS 授权记录...",
    restartAfterPrivacy: "完成授权后，请退出并重新打开 BeforePaste；请以 Doctor 状态为准。",
    resetPermissions: "重置 macOS 授权",
    resetPermissionsConfirm: "这会清除 BeforePaste 在辅助功能、输入监控、粘贴事件和自动化中的授权记录。是否继续？",
    resetPermissionsDone: "已清除 BeforePaste 的旧授权记录。请退出并重新打开 BeforePaste，然后在诊断页重新请求授权。",
    resetPermissionsFailed: "重置授权失败",
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
    restartCopy: "请重启 BeforePaste。如果是更新 preview 版本后出现授权异常，请重置 macOS 授权并重新授权。",
    safeShortcutReady: "安全粘贴可用",
    noAiTargetReady: "已就绪，当前不是 AI 目标",
    lastPasteNone: "暂无粘贴记录",
    manualTarget: "手动指定目标",
    autoDetection: "自动",
    testWaitingTitle: "等待测试",
    testWaitingCopy: "先复制测试内容，再粘贴到测试框。",
    testCopiedTitle: "测试内容已复制",
    testCopiedCopy: "点击测试框后按 Cmd+V。这里也支持安全粘贴快捷键。",
    testSuccessTitle: "🎉 已生效",
    testSuccessCopy: "BeforePaste 已经在本机完成脱敏，测试内容中的密钥被替换了。",
    testRawTitle: "还没有生效",
    testRawCopy: "你看到的是原始测试内容，请检查输入监控授权，或改用安全粘贴快捷键。",
    testChangedTitle: "粘贴结果不完整",
    testChangedCopy: "当前内容还不是预期的脱敏结果，请重新复制测试内容再试一次。",
    updateCheckingTitle: "正在检查更新",
    updateCheckingCopy: "BeforePaste 会检查 GitHub Releases；只有发现新版本时才提醒你下载。",
    updateReadyTitle: "已是最新版本",
    updateReadyCopy: "当前版本：{current}。",
    updateAvailableTitle: "发现新版本",
    updateAvailableCopy: "{current} → {latest}。请从 GitHub Releases 下载最新 preview 版本。",
    updateSkippedTitle: "已跳过这个版本",
    updateSkippedCopy: "{latest} 已隐藏；下一个版本发布后会再次提醒。",
    updateFailedTitle: "暂时无法检查更新",
    updateFailedCopy: "如果需要新版，可以手动打开 GitHub Releases 下载。",
    checkUpdate: "检查更新",
    downloadUpdate: "下载最新版本",
    skipUpdate: "跳过这个版本",
    remindLater: "稍后提醒",
    remindLaterDone: "已暂时隐藏更新提醒。",
    skipUpdateDone: "已跳过这个版本。",
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
  fields.setupTitle.textContent = currentLang === "ZH" ? "完成设置后，粘贴保护才会生效" : "Finish setup to protect paste";
  fields.setupCopy.textContent = currentLang === "ZH"
    ? "BeforePaste 需要 macOS 授权、可用快捷键和 VS Code 插件状态都清楚，避免安装后实际没有保护。"
    : "BeforePaste needs macOS permissions, a working shortcut, and clear VS Code bridge status so it actually protects paste.";
  fields.setupOpenDoctor.textContent = currentLang === "ZH" ? "打开诊断" : "Open Doctor";
  fields.setupDismissPrompt.textContent = currentLang === "ZH" ? "下次不再提示" : "Do not remind me again";
  fields.setupPermissionsTitle.textContent = currentLang === "ZH" ? "macOS 授权" : "macOS permissions";
  fields.setupPermissionsCopy.textContent = currentLang === "ZH"
    ? "安全粘贴需要辅助功能；自动保护 Cmd+V 还需要输入监控。点击授权会触发 macOS 系统提示。"
    : "Safe Paste needs Accessibility; automatic Cmd+V also needs Input Monitoring. The permission button triggers the macOS prompt.";
  fields.setupOpenPermissions.textContent = tr("openSettings");
  fields.setupCmdvTitle.textContent = currentLang === "ZH" ? "自动保护 Cmd+V" : "Automatic Cmd+V";
  fields.setupCmdvCopy.textContent = currentLang === "ZH"
    ? "只在识别到白名单里的 AI 应用、网页或终端时接管普通粘贴。"
    : "Protects normal paste only when an enabled AI app, website, or terminal is detected.";
  fields.setupSafeTitle.textContent = currentLang === "ZH" ? "安全粘贴快捷键" : "Safe paste shortcut";
  fields.setupSafeCopy.textContent = currentLang === "ZH"
    ? "目标识别不确定时使用它；无论当前应用是什么，都会粘贴脱敏后的内容。"
    : "Use this when target detection is unavailable or uncertain; it always pastes a redacted copy.";
  fields.setupVscodeTitle.textContent = currentLang === "ZH" ? "VS Code 插件" : "VS Code extension";
  fields.setupVscodeCopy.textContent = currentLang === "ZH"
    ? "用于识别 VS Code 集成终端中的 Codex、Claude Code、Gemini CLI；插件侧边栏/Chat 面板建议使用安全粘贴快捷键。"
    : "Required for Codex, Claude Code, and Gemini CLI in VS Code integrated terminals. Use Safe Paste for extension sidebars or chat panels.";
  fields.setupInstallVscodeBridge.textContent = tr("installExtension");
  fields.setupSkipVscodeBridge.textContent = tr("skipVscode");
  fields.doctorVscodeTitle.textContent = currentLang === "ZH" ? "VS Code 插件" : "VS Code extension";
  fields.doctorVscodeCopy.textContent = currentLang === "ZH"
    ? "如果你会粘贴到 VS Code 集成终端里的 Codex、Claude Code 或 Gemini CLI，请安装此插件。"
    : "Install this if you paste into Codex, Claude Code, or Gemini CLI inside VS Code integrated terminals.";
  fields.doctorInstallVscodeBridge.textContent = tr("installExtension");
  fields.pasteTestTitle.textContent = currentLang === "ZH" ? "验证保护效果" : "Try a protected paste";
  fields.pasteTestCopy.textContent = currentLang === "ZH"
    ? "复制一段安全的测试内容，然后在右侧测试框按 Cmd+V，确认能看到脱敏后的占位符。"
    : "Copy a safe sample, then press Cmd+V in the test box to confirm redaction.";
  fields.copyTestPayload.textContent = tr("copySample");
  fields.pasteTestSourceLabel.textContent = currentLang === "ZH" ? "测试内容源" : "Sample source";
  fields.pasteTestOutputLabel.textContent = currentLang === "ZH" ? "在这里粘贴" : "Paste here";
  fields.pasteTestOutput.placeholder = currentLang === "ZH"
    ? "点击“复制测试内容”，再在这里按 Cmd+V"
    : "Click Copy sample, then press Cmd+V here";
  fields.pasteTestNote.textContent = currentLang === "ZH"
    ? "这个测试框会被临时当作 BeforePaste 测试目标，因此可以直接验证普通 Cmd+V；安全粘贴快捷键也同样可用。"
    : "This box is treated as a temporary BeforePaste test target, so normal Cmd+V can be verified here. Safe Paste works too.";
  renderPasteTestResult("idle");

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
    currentLang === "ZH" ? "桌面端更新" : "Desktop updates",
    currentLang === "ZH" ? "正在检查" : "Checking protection",
    currentLang === "ZH" ? "权限状态" : "Permission checks",
    currentLang === "ZH" ? "辅助功能" : "Accessibility",
    currentLang === "ZH" ? "输入监控（Cmd+V 自动保护）" : "Input Monitoring (Cmd+V protection)",
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
    currentLang === "ZH" ? "检查 GitHub Releases；发现新版本时提醒你手动下载，不会自动安装。" : "Checks GitHub Releases and reminds you to download newer builds. It does not auto-install updates.",
    currentLang === "ZH" ? "检查粘贴模式、快捷键、目标识别和权限状态。" : "Checks the selected paste mode, shortcuts, target detection, and permissions.",
    currentLang === "ZH" ? "更新 preview 版本后，如授权状态异常，请删除旧授权并重新授权。" : "After a preview update, reset old macOS permission records if Doctor still reports missing access.",
    currentLang === "ZH" ? "用于执行最后一步粘贴。系统设置里看到已授权，不代表当前这份 app 已被 macOS 接受；请以 Doctor 状态为准。" : "Needed to perform paste actions. System Settings may show BeforePaste as enabled even when this app build is not accepted; trust Doctor status.",
    currentLang === "ZH" ? "用于自动保护普通 Cmd+V。点击“请求授权”会触发 macOS 输入监控提示；BeforePaste 不能自动替你授权。" : "Needed for automatic Cmd+V protection. Requesting access triggers the macOS Input Monitoring prompt; BeforePaste cannot grant itself access.",
    currentLang === "ZH" ? "用于识别浏览器标签页和终端上下文。如更新后状态异常，请重置 macOS 授权并重新授权。" : "Needed to read browser tab and terminal context. If access looks wrong after updating, reset macOS permissions and grant access again.",
    currentLang === "ZH" ? "只显示状态摘要，不会展示剪贴板内容。" : "Status summary only. Secret text is never shown here.",
    currentLang === "ZH" ? "默认跟随系统语言，也可以在这里手动切换。" : "Use the system language when available, or choose a language here.",
    currentLang === "ZH" ? "下次登录系统后自动打开 BeforePaste。" : "Start BeforePaste the next time you sign in.",
    currentLang === "ZH" ? "仅用于排查自动识别问题；30 分钟后自动恢复。" : "Temporary override for AI CLIs when auto detection misses. It expires after 30 minutes.",
  ]);

  document.querySelector(".bp-status-pill.is-on").textContent = tr("recommended");
  document.querySelector("#clear-target").textContent = tr("autoDetection");
  fields.checkUpdate.textContent = tr("checkUpdate");
  fields.downloadUpdate.textContent = tr("downloadUpdate");
  fields.skipUpdate.textContent = tr("skipUpdate");
  fields.remindUpdateLater.textContent = tr("remindLater");
  const styleOptions = fields.redactStyle?.querySelectorAll("option") || [];
  const styleLabels = currentLang === "ZH"
    ? ["固定标记", "类型标签", "示例占位", "直接删除"]
    : ["Marker", "Typed labels", "Sample values", "Remove values"];
  styleOptions.forEach((option, index) => {
    if (styleLabels[index]) option.textContent = styleLabels[index];
  });
  fields.installVscodeBridge.textContent = tr("installExtension");
  fields.doctorRefresh.textContent = currentLang === "ZH" ? "刷新状态" : "Refresh status";
  fields.doctorResetPermissions.textContent = tr("resetPermissions");
  setText("#doctor-reset-title", currentLang === "ZH" ? "Preview 版本授权重置" : "Preview build permission reset");
  setText(
    "#doctor-reset-copy",
    currentLang === "ZH"
      ? "更新 preview 版本后，如授权状态异常，请删除 BeforePaste 的旧授权记录并重新授权。"
      : "If a newly downloaded preview build still appears unauthorized, remove the old macOS permission records and grant access again.",
  );
  setText(
    "#doctor-reset-note",
    currentLang === "ZH"
      ? "系统设置里看到已授权，不代表当前这份 app 已被 macOS 接受；请以 Doctor 状态为准。"
      : "System Settings can show BeforePaste as enabled even when macOS has not accepted this exact app build. Trust Doctor status.",
  );
  for (const button of document.querySelectorAll("[data-open-privacy]")) {
    button.textContent = button.dataset.openPrivacy === "input_monitoring"
      ? (currentLang === "ZH" ? "请求授权" : "Request")
      : (currentLang === "ZH" ? "打开" : "Open");
  }
  renderUpdateStatus(lastUpdateStatus);
}

function renderConfig(config, platform = currentPlatform) {
  currentConfig = config;
  currentPlatform = platform || "macos";
  currentLang = languageFromConfig(config);
  fields.language.value = currentLang;
  applyStaticCopy();
  fields.setupDismissPrompt.hidden = Boolean(config.setup_prompt_dismissed);
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
  fields.checkUpdates.checked = Boolean(config.check_for_updates);
  fields.autoInstall.checked = Boolean(config.auto_install);
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
  } else if (nextPanel === "updates" && currentConfig?.check_for_updates) {
    checkLatestVersion({ silent: true }).catch((error) => setStatus(String(error)));
  } else if (pasteTestTargetActive) {
    setPasteTestTargetActive(false).catch((error) => setStatus(String(error)));
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
      ? `用于自动保护普通 ${label}。点击“请求授权”会触发 macOS 输入监控提示；BeforePaste 不能自动替你授权。`
      : `Needed for automatic ${label} protection. Requesting access triggers the macOS Input Monitoring prompt; BeforePaste cannot grant itself access.`;
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
  if (source === "test") {
    return currentLang === "ZH" ? "BeforePaste 测试框" : "BeforePaste test box";
  }
  return titleCase(detail || kind || reason);
}

function permissionLabel(value, key) {
  if (key && pendingPrivacyChecks.has(key)) {
    return [tr("restartPending"), "warn"];
  }
  if (value) {
    if (key) pendingPrivacyChecks.delete(key);
    return [tr("granted"), "ok"];
  }
  return [tr("missing"), "warn"];
}

function capabilityLabel(value, key) {
  if (key && pendingPrivacyChecks.has(key)) {
    return [tr("restartPending"), "warn"];
  }
  if (value) {
    if (key) pendingPrivacyChecks.delete(key);
    return [tr("available"), "ok"];
  }
  return [tr("unavailable"), "warn"];
}

function renderPermission(element, value, key) {
  const [label, state] = permissionLabel(value, key);
  setDiagnosticStatus(element, label, state);
}

function renderCapability(element, value, key) {
  const [label, state] = capabilityLabel(value, key);
  setDiagnosticStatus(element, label, state);
}

function setupLabel(ok, readyText = tr("ready"), missingText = tr("needsAttention")) {
  return [ok ? readyText : missingText, ok ? "ok" : "warn"];
}

function renderSetupChecklist() {
  if (!lastRuntimeStatus) return;
  const status = lastRuntimeStatus;
  const vscode = lastVscodeBridgeStatus;
  const hotkey = formatHotkeyForDisplay(status.force_paste_hotkey) || tr("notSet");
  const macos = status.platform === "macos";
  const accessibilityOk = !macos || status.permissions.accessibility;
  const inputMonitoringOk = !macos
    || !status.protect_normal_paste
    || status.permissions.input_monitoring;
  const permissionsPending = ["accessibility", "input_monitoring"].some((key) =>
    pendingPrivacyChecks.has(key)
  );
  const permissionsOk = accessibilityOk && inputMonitoringOk && !permissionsPending;
  const cmdvOk = status.beforepaste_enabled
    && macos
    && status.protect_normal_paste
    && permissionsOk
    && status.normal_paste_event_tap_installed;
  const safeOk = status.beforepaste_enabled && status.force_paste_hotkey_registered;
  const vscodeOk = Boolean(vscode?.installed);

  const [permissionsLabel, permissionsState] = permissionsPending
    ? [tr("restartPending"), "warn"]
    : setupLabel(
      permissionsOk,
      tr("granted"),
      currentLang === "ZH" ? "需要处理" : "Needs setup",
    );
  setDiagnosticStatus(fields.setupPermissionsStatus, permissionsLabel, permissionsState);
  fields.setupOpenPermissions.textContent = permissionsOk
    ? (currentLang === "ZH" ? "查看诊断" : "Doctor")
    : (!accessibilityOk
      ? tr("openSettings")
      : (currentLang === "ZH" ? "请求输入监控授权" : "Request Input Monitoring"));

  let cmdvLabel = currentLang === "ZH" ? "可用" : "Ready";
  let cmdvState = "ok";
  if (!status.beforepaste_enabled) {
    cmdvLabel = tr("disabled");
    cmdvState = "muted";
  } else if (!macos) {
    cmdvLabel = tr("notSupported");
    cmdvState = "muted";
  } else if (!status.protect_normal_paste) {
    cmdvLabel = tr("off");
    cmdvState = "warn";
  } else if (!permissionsOk || !status.normal_paste_event_tap_installed) {
    cmdvLabel = currentLang === "ZH" ? "需要处理" : "Needs setup";
    cmdvState = "warn";
  }
  setDiagnosticStatus(fields.setupCmdvStatus, cmdvLabel, cmdvState);

  const [safeLabel, safeState] = setupLabel(
    safeOk,
    currentLang === "ZH" ? `${hotkey} 可用` : `${hotkey} ready`,
    currentLang === "ZH" ? `${hotkey} 未注册` : `${hotkey} not registered`,
  );
  setDiagnosticStatus(fields.setupSafeStatus, safeLabel, safeState);

  let vscodeLabel = tr("checking");
  let vscodeState = "muted";
  const vscodeDismissed = Boolean(currentConfig?.vscode_bridge_dismissed);
  if (vscode) {
    vscodeLabel = vscode.installed
      ? tr("installed")
      : (vscodeDismissed ? (currentLang === "ZH" ? "已忽略" : "Ignored") : (currentLang === "ZH" ? "建议安装" : "Recommended"));
    vscodeState = vscode.installed ? "ok" : (vscodeDismissed ? "muted" : "warn");
    fields.setupInstallVscodeBridge.hidden = vscode.installed || vscodeDismissed || !vscode.vsix_path;
    fields.setupInstallVscodeBridge.title = vscode.installed
      ? (vscode.message || "")
      : (vscode.install_command || vscode.message || "");
    fields.setupSkipVscodeBridge.hidden = vscode.installed || vscodeDismissed;
  } else {
    fields.setupInstallVscodeBridge.hidden = true;
    fields.setupSkipVscodeBridge.hidden = vscodeDismissed;
  }
  setDiagnosticStatus(fields.setupVscodeStatus, vscodeLabel, vscodeState);

  const blocking = !status.beforepaste_enabled
    || !permissionsOk
    || (status.protect_normal_paste && !cmdvOk)
    || !safeOk;
  fields.setupCard.classList.toggle("is-ok", !blocking);
  fields.setupCard.classList.toggle("is-warn", blocking);
  if (blocking) {
    fields.setupTitle.textContent = currentLang === "ZH"
      ? "还差几步，BeforePaste 才能开始保护"
      : "BeforePaste still needs setup";
    fields.setupCopy.textContent = currentLang === "ZH"
      ? "请先确认 macOS 授权、Cmd+V 自动保护和安全粘贴快捷键。否则安装了也可能没有实际保护。"
      : "Confirm macOS permissions, automatic Cmd+V protection, and the safe paste shortcut before relying on it.";
  } else if (!vscodeOk && !vscodeDismissed) {
    fields.setupTitle.textContent = currentLang === "ZH"
      ? (status.protect_normal_paste
        ? "基础保护已就绪；建议安装 VS Code 插件"
        : "安全粘贴已就绪；建议安装 VS Code 插件")
      : (status.protect_normal_paste
        ? "Core protection is ready; install the VS Code extension"
        : "Safe Paste is ready; install the VS Code extension");
    fields.setupCopy.textContent = currentLang === "ZH"
      ? "安装插件后，BeforePaste 才能识别 VS Code 集成终端里的 AI CLI。插件侧边栏或 Chat 面板仍建议使用安全粘贴快捷键。"
      : "Install the extension to detect AI CLIs in VS Code integrated terminals. Use Safe Paste for extension sidebars or chat panels.";
  } else {
    fields.setupTitle.textContent = currentLang === "ZH"
      ? (status.protect_normal_paste ? "粘贴保护已就绪" : "安全粘贴已就绪")
      : (status.protect_normal_paste ? "Paste protection is ready" : "Safe Paste is ready");
    fields.setupCopy.textContent = currentLang === "ZH"
      ? (status.protect_normal_paste
        ? "Cmd+V 自动保护、安全粘贴快捷键和 VS Code 集成终端识别都已可用。"
        : "普通 Cmd+V 不会被接管；需要脱敏时请使用安全粘贴快捷键。")
      : (status.protect_normal_paste
        ? "Automatic Cmd+V protection, safe paste, and VS Code integrated terminal detection are ready."
        : "Normal paste is unchanged; use the Safe Paste shortcut when you need redaction.");
  }
}

function renderDoctor(status) {
  lastRuntimeStatus = status;
  currentPlatform = status.platform || currentPlatform;
  applyPlatformCopy(currentPlatform);
  renderPermission(fields.doctorAccessibility, status.permissions.accessibility, "accessibility");
  renderPermission(fields.doctorInputMonitoring, status.permissions.input_monitoring, "input_monitoring");
  renderPermission(fields.doctorAutomation, status.permissions.automation, "automation");

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
    const pending = ["accessibility", "input_monitoring"].some((key) => pendingPrivacyChecks.has(key));
    if (pending) {
      summaryCopy = currentLang === "ZH"
        ? "已打开 macOS 授权页。完成授权后，请退出并重新打开 BeforePaste；请以 Doctor 状态为准。"
        : "macOS Privacy settings were opened. After granting access, quit and reopen BeforePaste. Trust Doctor status.";
    } else if (missing.length) {
      summaryCopy = currentLang === "ZH"
        ? `请在 macOS 隐私设置中开启${missing.join("和")}。如果更新 preview 版本后仍异常，请先重置 macOS 授权再重新授权。`
        : `Grant ${missing.join(" and ")} in macOS Privacy settings. If this started after a preview update, reset macOS permissions first.`;
    } else {
      summaryCopy = tr("restartCopy");
    }
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
  renderSetupChecklist();
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
    renderVscodeBridgeStatus(status);
  } catch (error) {
    setDiagnosticStatus(fields.vscodeBridgeStatus, String(error), "warn");
    setDiagnosticStatus(fields.doctorVscodeBridge, String(error), "warn");
    lastVscodeBridgeStatus = {
      installed: false,
      vsix_path: null,
      install_command: "",
      message: String(error),
    };
    renderSetupChecklist();
  }
}

function renderVscodeBridgeStatus(status) {
  lastVscodeBridgeStatus = status;
  const label = status.installed ? tr("installed") : tr("notInstalled");
  const state = status.installed ? "ok" : "warn";
  setDiagnosticStatus(fields.vscodeBridgeStatus, label, state);
  setDiagnosticStatus(fields.doctorVscodeBridge, label, state);
  const canInstall = Boolean(status.vsix_path) && !status.installed;
  fields.installVscodeBridge.hidden = !canInstall;
  fields.installVscodeBridge.title = canInstall
    ? (status.install_command || "")
    : (status.message || "");
  fields.doctorInstallVscodeBridge.hidden = !canInstall;
  fields.doctorInstallVscodeBridge.title = fields.installVscodeBridge.title;
  renderSetupChecklist();
}

function formatCopy(template, values) {
  return String(template || "").replace(/\{(\w+)\}/g, (_, key) => values?.[key] ?? "");
}

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
    setUpdateCard(tr("updateCheckingTitle"), tr("updateCheckingCopy"), "muted");
    return;
  }

  const current = status.current_version || currentConfig?.version || "";
  const latest = status.version || "";
  if (status.available && status.skipped) {
    setUpdateCard(
      tr("updateSkippedTitle"),
      formatCopy(tr("updateSkippedCopy"), { current, latest }),
      "muted",
    );
    return;
  }
  if (status.available) {
    setUpdateCard(
      tr("updateAvailableTitle"),
      formatCopy(tr("updateAvailableCopy"), { current, latest }),
      "warn",
    );
    const url = status.download_url || status.html_url;
    fields.downloadUpdate.hidden = !url;
    fields.downloadUpdate.dataset.url = url || "";
    fields.skipUpdate.hidden = !latest;
    fields.skipUpdate.dataset.version = latest;
    fields.remindUpdateLater.hidden = false;
    return;
  }
  setUpdateCard(
    tr("updateReadyTitle"),
    formatCopy(tr("updateReadyCopy"), { current, latest }),
    "ok",
  );
}

function renderUpdateFailure(error) {
  lastUpdateStatus = null;
  fields.downloadUpdate.hidden = false;
  fields.downloadUpdate.dataset.url = "https://github.com/beforewire/beforepaste/releases/latest";
  fields.skipUpdate.hidden = true;
  fields.remindUpdateLater.hidden = true;
  setUpdateCard(
    tr("updateFailedTitle"),
    `${tr("updateFailedCopy")} ${String(error || "")}`.trim(),
    "warn",
  );
}

async function checkLatestVersion({ silent = false } = {}) {
  renderUpdateStatus(null);
  fields.checkUpdate.disabled = true;
  try {
    const status = await invoke("check_for_update");
    renderUpdateStatus(status);
    if (!silent) {
      setStatus(status.available && !status.skipped ? tr("updateAvailable") : tr("statusRefreshed"));
    }
  } catch (error) {
    renderUpdateFailure(error);
    if (!silent) setStatus(String(error));
  } finally {
    fields.checkUpdate.disabled = false;
  }
}

function normalizeTestText(value) {
  return String(value || "").replace(/\r\n/g, "\n").trim();
}

function renderPasteTestResult(state) {
  const result = fields.pasteTestResult;
  if (!result) return;
  result.classList.remove("is-idle", "is-copied", "is-success", "is-warn");
  const className = {
    idle: "is-idle",
    copied: "is-copied",
    success: "is-success",
    raw: "is-warn",
    changed: "is-warn",
  }[state] || "is-idle";
  result.classList.add(className);
  const titleKey = {
    idle: "testWaitingTitle",
    copied: "testCopiedTitle",
    success: "testSuccessTitle",
    raw: "testRawTitle",
    changed: "testChangedTitle",
  }[state] || "testWaitingTitle";
  const copyKey = {
    idle: "testWaitingCopy",
    copied: "testCopiedCopy",
    success: "testSuccessCopy",
    raw: "testRawCopy",
    changed: "testChangedCopy",
  }[state] || "testWaitingCopy";
  fields.pasteTestResultTitle.textContent = tr(titleKey);
  fields.pasteTestResultCopy.textContent = tr(copyKey);
}

function evaluatePasteTestResult() {
  const pasted = normalizeTestText(fields.pasteTestOutput.value);
  if (!pasted) {
    renderPasteTestResult(lastTestPayloadStatus ? "idle" : "changed");
    return;
  }
  const expected = normalizeTestText(lastTestPayloadStatus?.redacted);
  const source = normalizeTestText(lastTestPayloadStatus?.source);
  if (expected && pasted === expected) {
    renderPasteTestResult("success");
    return;
  }
  if (source && pasted === source) {
    renderPasteTestResult("raw");
    return;
  }
  if (pasted.includes("sk-beforepaste-demo") || pasted.includes("beforepasteDemoSecret")) {
    renderPasteTestResult("raw");
    return;
  }
  if (/\[(?:REDACTED|[A-Z0-9_]{3,})\]/.test(pasted)) {
    renderPasteTestResult("success");
    return;
  }
  renderPasteTestResult("changed");
}

async function refreshPasteTestPayload() {
  try {
    lastTestPayloadStatus = await invoke("get_test_payload_status");
    fields.pasteTestSource.value = lastTestPayloadStatus.source || "";
    evaluatePasteTestResult();
  } catch (error) {
    fields.pasteTestSource.value = "";
    renderPasteTestResult("changed");
    setStatus(String(error));
  }
}

async function setPasteTestTargetActive(active) {
  if (pasteTestTargetActive === active) return;
  pasteTestTargetActive = active;
  try {
    await invoke("set_paste_test_target", { active });
    if (active) {
      await refreshDoctor();
    }
  } catch (error) {
    setStatus(String(error));
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
    check_for_updates: Boolean(currentConfig.check_for_updates),
    auto_install: Boolean(currentConfig.auto_install),
    setup_prompt_dismissed: Boolean(currentConfig.setup_prompt_dismissed),
    vscode_bridge_dismissed: Boolean(currentConfig.vscode_bridge_dismissed),
    lang: fields.language.value,
    redact_style: fields.redactStyle.value,
    redact_pattern: fields.redactPattern.value || "[REDACTED]",
    disabled_targets: currentConfig.disabled_targets || [],
    disabled_target_surfaces: currentConfig.disabled_target_surfaces || [],
  };
}

async function load() {
  const [config, catalog, cliCatalog, status, vscodeStatus, testPayloadStatus] = await Promise.all([
    invoke("get_config"),
    invoke("get_target_catalog"),
    invoke("get_cli_target_catalog"),
    invoke("get_runtime_status"),
    invoke("get_vscode_bridge_status"),
    invoke("get_test_payload_status"),
  ]);
  targetCatalog = catalog;
  cliTargetCatalog = cliCatalog;
  lastVscodeBridgeStatus = vscodeStatus;
  lastTestPayloadStatus = testPayloadStatus;
  fields.pasteTestSource.value = testPayloadStatus.source || "";
  renderConfig(config, status.platform);
  renderDoctor(status);
  renderVscodeBridgeStatus(vscodeStatus);
  evaluatePasteTestResult();
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
  fields.setupDismissPrompt.hidden = Boolean(config.setup_prompt_dismissed);
  await refreshPasteTestPayload();
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

fields.checkUpdate.addEventListener("click", () => {
  checkLatestVersion().catch((error) => setStatus(String(error)));
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
    renderUpdateStatus({
      ...lastUpdateStatus,
      skipped: true,
    });
    setStatus(tr("skipUpdateDone"));
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
  setStatus(tr("remindLaterDone"));
});

fields.doctorRefresh.addEventListener("click", async () => {
  await refreshDoctor();
  setStatus(tr("statusRefreshed"));
});

fields.setupOpenDoctor.addEventListener("click", () => {
  activatePanel("doctor");
});

fields.setupDismissPrompt.addEventListener("click", async () => {
  currentConfig = {
    ...currentConfig,
    setup_prompt_dismissed: true,
  };
  fields.setupDismissPrompt.hidden = true;
  try {
    await invoke("save_config", { config: collectConfig() });
    setStatus(tr("setupDismissed"));
  } catch (error) {
    currentConfig.setup_prompt_dismissed = false;
    fields.setupDismissPrompt.hidden = false;
    setStatus(String(error));
  }
});

fields.setupSkipVscodeBridge.addEventListener("click", async () => {
  currentConfig = {
    ...currentConfig,
    vscode_bridge_dismissed: true,
  };
  fields.setupSkipVscodeBridge.hidden = true;
  fields.setupInstallVscodeBridge.hidden = true;
  renderSetupChecklist();
  try {
    await invoke("save_config", { config: collectConfig() });
    setStatus(tr("vscodeDismissed"));
  } catch (error) {
    currentConfig.vscode_bridge_dismissed = false;
    renderSetupChecklist();
    setStatus(String(error));
  }
});

async function openPrivacyKind(privacyKind) {
  try {
    pendingPrivacyChecks.add(privacyKind);
    await invoke("open_privacy_settings", { kind: privacyKind });
    await refreshDoctor();
    setStatus(tr("restartAfterPrivacy"));
  } catch (error) {
    pendingPrivacyChecks.delete(privacyKind);
    setStatus(String(error));
  }
}

fields.setupOpenPermissions.addEventListener("click", async () => {
  const status = lastRuntimeStatus;
  if (!status || status.platform !== "macos") {
    activatePanel("doctor");
    return;
  }
  if (!status.permissions.accessibility) {
    await openPrivacyKind("accessibility");
    return;
  }
  if (status.protect_normal_paste && !status.permissions.input_monitoring) {
    await openPrivacyKind("input_monitoring");
    return;
  }
  activatePanel("doctor");
});

fields.doctorResetPermissions.addEventListener("click", async () => {
  fields.doctorResetPermissions.disabled = true;
  setStatus(tr("resettingPermissions"));
  try {
    await invoke("reset_macos_permissions");
    pendingPrivacyChecks.add("accessibility");
    pendingPrivacyChecks.add("input_monitoring");
    pendingPrivacyChecks.add("automation");
    await refreshDoctor();
    setStatus(tr("resetPermissionsDone"));
  } catch (error) {
    setStatus(`${tr("resetPermissionsFailed")}: ${String(error)}`);
  } finally {
    fields.doctorResetPermissions.disabled = false;
  }
});

async function installVscodeBridgeFromUi(button) {
  button.disabled = true;
  try {
    await invoke("install_vscode_bridge");
    await refreshVscodeBridge();
    setStatus(tr("installDone"));
  } catch (error) {
    setStatus(`${tr("installFailed")}: ${String(error)}`);
  } finally {
    button.disabled = false;
  }
}

fields.installVscodeBridge.addEventListener("click", async () => {
  await installVscodeBridgeFromUi(fields.installVscodeBridge);
});

fields.setupInstallVscodeBridge.addEventListener("click", async () => {
  await installVscodeBridgeFromUi(fields.setupInstallVscodeBridge);
});

fields.doctorInstallVscodeBridge.addEventListener("click", async () => {
  await installVscodeBridgeFromUi(fields.doctorInstallVscodeBridge);
});

fields.copyTestPayload.addEventListener("click", async () => {
  fields.copyTestPayload.disabled = true;
  try {
    await invoke("copy_test_payload");
    fields.pasteTestOutput.value = "";
    fields.pasteTestOutput.focus();
    await setPasteTestTargetActive(true);
    renderPasteTestResult("copied");
    setStatus(tr("sampleCopied"));
  } catch (error) {
    setStatus(String(error));
  } finally {
    fields.copyTestPayload.disabled = false;
  }
});

fields.pasteTestOutput.addEventListener("focus", () => {
  setPasteTestTargetActive(true).catch((error) => setStatus(String(error)));
});

fields.pasteTestOutput.addEventListener("blur", () => {
  setPasteTestTargetActive(false).catch((error) => setStatus(String(error)));
});

fields.pasteTestOutput.addEventListener("input", () => {
  evaluatePasteTestResult();
});

window.addEventListener("blur", () => {
  setPasteTestTargetActive(false).catch((error) => setStatus(String(error)));
});

for (const button of document.querySelectorAll("[data-open-privacy]")) {
  button.addEventListener("click", async () => {
    await openPrivacyKind(button.dataset.openPrivacy);
  });
}

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

window.addEventListener("focus", refreshDoctorIfVisible);
document.addEventListener("visibilitychange", () => {
  if (!document.hidden) {
    refreshDoctorIfVisible();
  }
});

load().catch((error) => {
  setStatus(String(error));
});
