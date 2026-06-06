use serde::Serialize;

use crate::config::Config;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct TargetCatalogEntry {
    pub id: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    pub web_domains: &'static [&'static str],
    pub macos_bundle_ids: &'static [&'static str],
    pub windows_process_names: &'static [&'static str],
    pub web_adapted: bool,
    pub app_adapted: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct CliTargetCatalogEntry {
    pub id: &'static str,
    pub label: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetSurface {
    App,
    Web,
    Terminal,
    Vscode,
}

impl TargetSurface {
    fn as_str(self) -> &'static str {
        match self {
            TargetSurface::App => "app",
            TargetSurface::Web => "web",
            TargetSurface::Terminal => "terminal",
            TargetSurface::Vscode => "vscode",
        }
    }
}

const OVERSEAS: &str = "Overseas AI";
const CHINA: &str = "China AI";

pub const CLI_TARGET_CATALOG: &[CliTargetCatalogEntry] = &[
    CliTargetCatalogEntry {
        id: "codex",
        label: "Codex",
    },
    CliTargetCatalogEntry {
        id: "claude",
        label: "Claude Code",
    },
    CliTargetCatalogEntry {
        id: "gemini",
        label: "Gemini CLI",
    },
    CliTargetCatalogEntry {
        id: "aider",
        label: "aider",
    },
    CliTargetCatalogEntry {
        id: "opencode",
        label: "OpenCode",
    },
];

pub const TARGET_CATALOG: &[TargetCatalogEntry] = &[
    TargetCatalogEntry {
        id: "chatgpt",
        label: "ChatGPT",
        group: OVERSEAS,
        web_domains: &["chatgpt.com", "chat.openai.com"],
        macos_bundle_ids: &["com.openai.chat"],
        windows_process_names: &[],
        web_adapted: true,
        app_adapted: true,
    },
    TargetCatalogEntry {
        id: "claude",
        label: "Claude",
        group: OVERSEAS,
        web_domains: &["claude.ai"],
        macos_bundle_ids: &["com.anthropic.claudefordesktop"],
        windows_process_names: &[],
        web_adapted: true,
        app_adapted: true,
    },
    TargetCatalogEntry {
        id: "gemini",
        label: "Gemini",
        group: OVERSEAS,
        web_domains: &["gemini.google.com", "aistudio.google.com"],
        macos_bundle_ids: &["com.google.GeminiMacOS"],
        windows_process_names: &[],
        web_adapted: true,
        app_adapted: true,
    },
    TargetCatalogEntry {
        id: "poe",
        label: "Poe",
        group: OVERSEAS,
        web_domains: &["poe.com"],
        macos_bundle_ids: &[],
        windows_process_names: &[],
        web_adapted: true,
        app_adapted: false,
    },
    TargetCatalogEntry {
        id: "perplexity",
        label: "Perplexity",
        group: OVERSEAS,
        web_domains: &["perplexity.ai"],
        macos_bundle_ids: &[],
        windows_process_names: &[],
        web_adapted: true,
        app_adapted: false,
    },
    TargetCatalogEntry {
        id: "copilot",
        label: "Microsoft Copilot",
        group: OVERSEAS,
        web_domains: &["copilot.microsoft.com"],
        macos_bundle_ids: &[],
        windows_process_names: &[],
        web_adapted: true,
        app_adapted: false,
    },
    TargetCatalogEntry {
        id: "grok",
        label: "Grok",
        group: OVERSEAS,
        web_domains: &["grok.com"],
        macos_bundle_ids: &[],
        windows_process_names: &[],
        web_adapted: true,
        app_adapted: false,
    },
    TargetCatalogEntry {
        id: "deepseek",
        label: "DeepSeek",
        group: CHINA,
        web_domains: &["deepseek.com", "chat.deepseek.com"],
        macos_bundle_ids: &[],
        windows_process_names: &[],
        web_adapted: true,
        app_adapted: false,
    },
    TargetCatalogEntry {
        id: "kimi",
        label: "Kimi",
        group: CHINA,
        web_domains: &["kimi.com", "kimi.moonshot.cn"],
        macos_bundle_ids: &[],
        windows_process_names: &[],
        web_adapted: true,
        app_adapted: false,
    },
    TargetCatalogEntry {
        id: "doubao",
        label: "Doubao",
        group: CHINA,
        web_domains: &["doubao.com"],
        macos_bundle_ids: &["com.bot.pc.doubao", "com.bot.pc.doubao.browser"],
        windows_process_names: &[],
        web_adapted: true,
        app_adapted: true,
    },
    TargetCatalogEntry {
        id: "qwen",
        label: "Qwen",
        group: CHINA,
        web_domains: &[
            "qwen.ai",
            "chat.qwen.ai",
            "qianwen.com",
            "tongyi.aliyun.com",
        ],
        macos_bundle_ids: &[],
        windows_process_names: &[],
        web_adapted: true,
        app_adapted: false,
    },
    TargetCatalogEntry {
        id: "minimax",
        label: "MiniMax / Hailuo",
        group: CHINA,
        web_domains: &["hailuoai.com", "minimax.io", "chat.minimax.io"],
        macos_bundle_ids: &[],
        windows_process_names: &[],
        web_adapted: true,
        app_adapted: false,
    },
    TargetCatalogEntry {
        id: "zhipu",
        label: "Zhipu / ChatGLM",
        group: CHINA,
        web_domains: &["chatglm.cn", "zhipuai.cn"],
        macos_bundle_ids: &[],
        windows_process_names: &[],
        web_adapted: true,
        app_adapted: false,
    },
];

pub fn catalog() -> &'static [TargetCatalogEntry] {
    TARGET_CATALOG
}

pub fn cli_catalog() -> &'static [CliTargetCatalogEntry] {
    CLI_TARGET_CATALOG
}

pub fn enabled(config: &Config, target_id: &str) -> bool {
    !config.disabled_targets.iter().any(|id| id == target_id)
}

pub fn enabled_on(config: &Config, surface: TargetSurface, target_id: &str) -> bool {
    if !enabled(config, target_id) {
        return false;
    }
    let key = format!("{}:{target_id}", surface.as_str());
    !config
        .disabled_target_surfaces
        .iter()
        .any(|disabled| disabled == &key)
}

#[cfg(target_os = "macos")]
pub fn match_macos_bundle(config: &Config, bundle_id: &str) -> Option<&'static TargetCatalogEntry> {
    TARGET_CATALOG.iter().find(|entry| {
        enabled_on(config, TargetSurface::App, entry.id)
            && entry.macos_bundle_ids.contains(&bundle_id)
    })
}

pub fn match_domain(
    config: &Config,
    host: &str,
) -> Option<(&'static TargetCatalogEntry, &'static str)> {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    TARGET_CATALOG.iter().find_map(|entry| {
        if !enabled_on(config, TargetSurface::Web, entry.id) {
            return None;
        }
        entry.web_domains.iter().find_map(|domain| {
            if host == *domain || host.ends_with(&format!(".{domain}")) {
                Some((entry, *domain))
            } else {
                None
            }
        })
    })
}
