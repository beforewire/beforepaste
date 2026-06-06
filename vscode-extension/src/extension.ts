import * as crypto from "crypto";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import * as vscode from "vscode";

const STATE_TTL_SECS = 45;
const HEARTBEAT_MS = 15_000;
const TERMINAL_APP = "vscode";
const KNOWN_AI_COMMANDS = new Set([
  "codex",
  "gemini",
  "claude",
  "aider",
  "continue",
  "opencode",
]);

type ActiveExecution = {
  cmd: string;
  kind: string;
  cwd: string;
};

const terminalIds = new WeakMap<vscode.Terminal, string>();
const activeExecutions = new Map<string, ActiveExecution>();
const stateFiles = new Map<string, string>();
let heartbeat: NodeJS.Timeout | undefined;
let windowFocused = true;
let windowId = "";
let output: vscode.OutputChannel;

export function activate(context: vscode.ExtensionContext): void {
  windowId = stableWindowId(context);
  output = vscode.window.createOutputChannel("BeforePaste");
  context.subscriptions.push(output);
  log(`activated extension=${context.extension.id} vscode=${vscode.version}`);

  context.subscriptions.push(
    vscode.commands.registerCommand("beforepaste.refreshTerminalState", () => {
      log("manual refresh requested");
      void writeActiveState();
    }),
    vscode.commands.registerCommand("beforepaste.showTerminalBridgeStatus", () => {
      logStatus(context);
      output.show(true);
    }),
    vscode.window.onDidChangeActiveTerminal(() => {
      void writeActiveState();
    }),
    vscode.window.tabGroups.onDidChangeTabs(() => {
      void writeActiveState();
    }),
    vscode.window.tabGroups.onDidChangeTabGroups(() => {
      void writeActiveState();
    }),
    vscode.window.onDidChangeWindowState((state) => {
      windowFocused = state.focused;
      void writeActiveState();
    }),
    vscode.window.onDidCloseTerminal((terminal) => {
      const terminalId = terminalIds.get(terminal);
      if (terminalId) {
        activeExecutions.delete(terminalId);
        removeStateFile(terminalId);
      }
    }),
  );

  registerShellExecutionEvents(context);

  heartbeat = setInterval(() => {
    void writeActiveState();
  }, HEARTBEAT_MS);

  void writeActiveState();
}

export function deactivate(): void {
  if (heartbeat) {
    clearInterval(heartbeat);
  }
  for (const terminalId of stateFiles.keys()) {
    removeStateFile(terminalId);
  }
}

function registerShellExecutionEvents(context: vscode.ExtensionContext): void {
  const windowApi = vscode.window as typeof vscode.window & {
    onDidStartTerminalShellExecution?: (
      listener: (event: TerminalShellExecutionStartEvent) => void,
    ) => vscode.Disposable;
    onDidEndTerminalShellExecution?: (
      listener: (event: TerminalShellExecutionEndEvent) => void,
    ) => vscode.Disposable;
  };

  if (typeof windowApi.onDidStartTerminalShellExecution === "function") {
    log("terminal shell execution start event is available");
    context.subscriptions.push(
      windowApi.onDidStartTerminalShellExecution((event) => {
        const cmd = commandLineValue(event.execution.commandLine);
        const kind = classifyCommand(cmd) ?? classifyCommand(event.terminal.name);
        const terminalId = idForTerminal(event.terminal);
        log(
          `shell start terminal=${event.terminal.name} id=${terminalId} command=${JSON.stringify(cmd)} kind=${kind ?? "none"}`,
        );
        if (kind) {
          activeExecutions.set(terminalId, {
            cmd: cmd || event.terminal.name,
            kind,
            cwd: cwdForExecution(event.execution, event.terminal),
          });
        } else {
          activeExecutions.delete(terminalId);
          removeStateFile(terminalId);
        }
        void writeActiveState();
      }),
    );
  } else {
    log("terminal shell execution start event is not available");
  }

  if (typeof windowApi.onDidEndTerminalShellExecution === "function") {
    log("terminal shell execution end event is available");
    context.subscriptions.push(
      windowApi.onDidEndTerminalShellExecution((event) => {
        const terminalId = idForTerminal(event.terminal);
        log(`shell end terminal=${event.terminal.name} id=${terminalId}`);
        activeExecutions.delete(terminalId);
        removeStateFile(terminalId);
        void writeActiveState();
      }),
    );
  } else {
    log("terminal shell execution end event is not available");
  }
}

async function writeActiveState(): Promise<void> {
  if (!windowFocused) {
    removeAllStateFiles();
    return;
  }

  const activeTerminal = vscode.window.activeTerminal;
  if (!activeTerminal) {
    await writeAiViewState();
    return;
  }

  const activeTerminalId = idForTerminal(activeTerminal);
  for (const terminalId of [...stateFiles.keys()]) {
    if (terminalId !== activeTerminalId) {
      removeStateFile(terminalId);
    }
  }

  const execution =
    activeExecutions.get(activeTerminalId) ?? executionFromTerminalName(activeTerminal);
  if (!execution) {
    log(`no active AI execution for terminal=${activeTerminal.name} id=${activeTerminalId}`);
    removeStateFile(activeTerminalId);
    await writeAiViewState();
    return;
  }

  const now = Math.floor(Date.now() / 1000);
  const target = {
    tty: `vscode:${windowId}:${activeTerminalId}`,
    cmd: execution.cmd,
    kind: execution.kind,
    cwd: execution.cwd,
    terminal_app: TERMINAL_APP,
    terminal_id: activeTerminalId,
    vscode_window_id: windowId,
    vscode_terminal_id: activeTerminalId,
    updated_at: now,
    expires_at: now + STATE_TTL_SECS,
  };

  const file = statePath(activeTerminalId);
  await fs.promises.mkdir(path.dirname(file), { recursive: true });
  await fs.promises.writeFile(file, `${JSON.stringify(target, null, 2)}\n`, {
    mode: 0o600,
  });
  stateFiles.set(activeTerminalId, file);
  log(`wrote state file=${file} kind=${execution.kind}`);
}

async function writeAiViewState(): Promise<void> {
  const view = activeAiView();
  if (!view) {
    removeStateFile("ai-view");
    return;
  }

  for (const terminalId of [...stateFiles.keys()]) {
    if (terminalId !== "ai-view") {
      removeStateFile(terminalId);
    }
  }

  const now = Math.floor(Date.now() / 1000);
  const target = {
    tty: `vscode:${windowId}:ai-view`,
    cmd: view.cmd,
    kind: view.kind,
    cwd: vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? os.homedir(),
    terminal_app: TERMINAL_APP,
    terminal_id: "ai-view",
    vscode_window_id: windowId,
    vscode_terminal_id: "ai-view",
    updated_at: now,
    expires_at: now + STATE_TTL_SECS,
  };

  const file = statePath("ai-view");
  await fs.promises.mkdir(path.dirname(file), { recursive: true });
  await fs.promises.writeFile(file, `${JSON.stringify(target, null, 2)}\n`, {
    mode: 0o600,
  });
  stateFiles.set("ai-view", file);
  log(`wrote VS Code AI view state file=${file} kind=${view.kind} source=${view.source}`);
}

function removeAllStateFiles(): void {
  for (const terminalId of [...stateFiles.keys()]) {
    removeStateFile(terminalId);
  }
}

function removeStateFile(terminalId: string): void {
  const file = stateFiles.get(terminalId) ?? statePath(terminalId);
  stateFiles.delete(terminalId);
  fs.rm(file, { force: true }, () => undefined);
}

function idForTerminal(terminal: vscode.Terminal): string {
  const existing = terminalIds.get(terminal);
  if (existing) {
    return existing;
  }
  const id = crypto.randomUUID();
  terminalIds.set(terminal, id);
  return id;
}

function stableWindowId(context: vscode.ExtensionContext): string {
  const workspace = vscode.workspace.workspaceFolders
    ?.map((folder) => folder.uri.toString())
    .join("|") ?? "empty";
  const source = `${context.extension.id}|${vscode.env.appName}|${workspace}`;
  return crypto.createHash("sha256").update(source).digest("hex").slice(0, 16);
}

function commandLineValue(commandLine: unknown): string {
  if (typeof commandLine === "string") {
    return commandLine;
  }
  if (
    commandLine &&
    typeof commandLine === "object" &&
    "value" in commandLine &&
    typeof (commandLine as { value: unknown }).value === "string"
  ) {
    return (commandLine as { value: string }).value;
  }
  return "";
}

function classifyCommand(commandLine: string): string | undefined {
  for (const rawWord of commandLine.split(/\s+/)) {
    const word = rawWord.replace(/^['"`]+|['"`]+$/g, "");
    if (!word || shouldSkipWord(word)) {
      continue;
    }
    return classifyBinaryName(basenameAnyPlatform(word));
  }
  return undefined;
}

function classifyBinaryName(command: string): string | undefined {
  const normalized = command
    .trim()
    .toLowerCase()
    .replace(/\.(exe|cmd|bat)$/u, "");
  if (normalized.includes(".")) {
    return undefined;
  }
  for (const kind of KNOWN_AI_COMMANDS) {
    if (
      normalized === kind ||
      normalized.startsWith(`${kind}-`) ||
      normalized.startsWith(`${kind}_`)
    ) {
      return kind;
    }
  }
  return undefined;
}

function basenameAnyPlatform(word: string): string {
  const normalized = word.replace(/\\/gu, "/");
  return path.basename(normalized);
}

function shouldSkipWord(word: string): boolean {
  return (
    word.includes("=") ||
    word.startsWith("-") ||
    ["command", "builtin", "exec", "noglob", "env", "sudo"].includes(word)
  );
}

function cwdForExecution(
  execution: TerminalShellExecution,
  terminal: vscode.Terminal,
): string {
  const executionCwd = uriPath((execution as { cwd?: unknown }).cwd);
  if (executionCwd) {
    return executionCwd;
  }
  const shellCwd = uriPath(terminal.shellIntegration?.cwd);
  if (shellCwd) {
    return shellCwd;
  }
  return vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? os.homedir();
}

function executionFromTerminalName(terminal: vscode.Terminal): ActiveExecution | undefined {
  const kind = classifyCommand(terminal.name);
  if (!kind) {
    return undefined;
  }
  log(`classified active terminal name=${terminal.name} kind=${kind}`);
  return {
    cmd: terminal.name,
    kind,
    cwd: cwdForTerminal(terminal),
  };
}

function activeAiView(): { kind: string; cmd: string; source: string } | undefined {
  const tab = vscode.window.tabGroups.activeTabGroup.activeTab;
  if (!tab) {
    return undefined;
  }
  const source = activeTabSource(tab);
  if (!source) {
    return undefined;
  }
  const kind = classifyAiSurface(source);
  if (!kind) {
    return undefined;
  }
  log(`classified active AI tab label=${tab.label} source=${source} kind=${kind}`);
  return {
    kind,
    cmd: source,
    source,
  };
}

function activeTabSource(tab: vscode.Tab): string | undefined {
  const input = tab.input;
  if (input instanceof vscode.TabInputWebview) {
    return `webview:${input.viewType}:${tab.label}`;
  }
  if (input instanceof vscode.TabInputCustom) {
    return `custom:${input.viewType}:${tab.label}`;
  }
  return undefined;
}

function classifyAiSurface(source: string): string | undefined {
  const normalized = source.toLowerCase();
  for (const kind of KNOWN_AI_COMMANDS) {
    if (normalized.includes(kind)) {
      return kind;
    }
  }
  if (normalized.includes("anthropic")) {
    return "claude";
  }
  if (normalized.includes("openai")) {
    return "codex";
  }
  if (normalized.includes("google") && normalized.includes("ai")) {
    return "gemini";
  }
  return undefined;
}

function cwdForTerminal(terminal: vscode.Terminal): string {
  const shellCwd = uriPath(terminal.shellIntegration?.cwd);
  if (shellCwd) {
    return shellCwd;
  }
  return vscode.workspace.workspaceFolders?.[0]?.uri.fsPath ?? os.homedir();
}

function uriPath(value: unknown): string | undefined {
  if (value instanceof vscode.Uri) {
    return value.fsPath;
  }
  return undefined;
}

function statePath(terminalId: string): string {
  const safeId = terminalId.replace(/[^A-Za-z0-9_.-]/g, "_");
  return path.join(configBaseDir(), "terminal-targets", `vscode_${windowId}_${safeId}.json`);
}

function configBaseDir(): string {
  const override = process.env.BEFOREPASTE_CONFIG_HOME;
  if (override && override.trim()) {
    return path.join(override, "beforewire", "beforepaste");
  }

  if (process.platform === "darwin") {
    return path.join(os.homedir(), "Library", "Application Support", "beforewire", "beforepaste");
  }
  if (process.platform === "win32") {
    return path.join(
      process.env.APPDATA ?? path.join(os.homedir(), "AppData", "Roaming"),
      "beforewire",
      "beforepaste",
    );
  }
  return path.join(
    process.env.XDG_CONFIG_HOME ?? path.join(os.homedir(), ".config"),
    "beforewire",
    "beforepaste",
  );
}

function log(message: string): void {
  output?.appendLine(`[${new Date().toISOString()}] ${message}`);
}

function logStatus(context: vscode.ExtensionContext): void {
  const windowApi = vscode.window as typeof vscode.window & {
    onDidStartTerminalShellExecution?: unknown;
    onDidEndTerminalShellExecution?: unknown;
  };
  const activeTerminal = vscode.window.activeTerminal;
  log("status");
  log(`extension=${context.extension.id}`);
  log(`window_id=${windowId}`);
  log(`config_dir=${configBaseDir()}`);
  log(`window_focused=${windowFocused}`);
  log(`active_terminal=${activeTerminal?.name ?? "(none)"}`);
  log(`active_tab=${activeTabStatus()}`);
  log(`active_execution_count=${activeExecutions.size}`);
  log(`state_file_count=${stateFiles.size}`);
  log(`start_event_available=${typeof windowApi.onDidStartTerminalShellExecution === "function"}`);
  log(`end_event_available=${typeof windowApi.onDidEndTerminalShellExecution === "function"}`);
  for (const [terminalId, execution] of activeExecutions.entries()) {
    log(
      `execution id=${terminalId} kind=${execution.kind} cmd=${JSON.stringify(execution.cmd)} cwd=${execution.cwd}`,
    );
  }
}

function activeTabStatus(): string {
  const tab = vscode.window.tabGroups.activeTabGroup.activeTab;
  if (!tab) {
    return "(none)";
  }
  const source = activeTabSource(tab) ?? `non-webview:${tab.label}`;
  return `${source} active=${tab.isActive}`;
}

type TerminalShellExecution = {
  commandLine: unknown;
  cwd?: vscode.Uri;
};

type TerminalShellExecutionStartEvent = {
  terminal: vscode.Terminal;
  execution: TerminalShellExecution;
};

type TerminalShellExecutionEndEvent = {
  terminal: vscode.Terminal;
  execution: TerminalShellExecution;
};
