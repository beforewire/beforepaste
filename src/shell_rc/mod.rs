use std::path::{Path, PathBuf};

use crate::config::atomic_write;

pub const BLOCK_BEGIN: &str = "# ---- beforepaste shell integration ----";
pub const BLOCK_END: &str = "# ---- beforepaste shell integration ----";
const LEGACY_BLOCK_BEGIN: &str = "# ---- beforepaste paste-guard ----";
const LEGACY_BLOCK_END: &str = "# ---- beforepaste paste-guard ----";

/// Substring that uniquely identifies a paste-guard alias line in the user's
/// rc file. `uninstall` falls back to this when the fence comments have
/// been stripped or edited.
pub const ALIAS_MARKER: &str = "beforepaste paste-guard --";
pub const HOOK_MARKER: &str = "beforepaste terminal-enter";

const ENV_NO_OS: &str = "BEFOREPASTE_NO_OS_SIDE_EFFECTS";

fn os_side_effects_allowed() -> bool {
    std::env::var_os(ENV_NO_OS).is_none()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shell {
    Bash,
    Zsh,
    Fish,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleMode {
    Preserve,
    Keepalive,
}

const STATE_TTL_SECS: u64 = 12 * 60 * 60;

impl Shell {
    /// Pick the shell from `$SHELL`. `None` on Windows, an unknown shell, or
    /// when the variable is missing - the caller falls back to printing
    /// generic instructions in those cases.
    pub fn from_env() -> Option<Self> {
        let raw = std::env::var("SHELL").ok()?;
        let name = Path::new(&raw).file_name()?.to_string_lossy().to_string();
        Self::from_name(&name)
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "bash" => Some(Shell::Bash),
            "zsh" => Some(Shell::Zsh),
            "fish" => Some(Shell::Fish),
            _ => None,
        }
    }

    pub fn rc_path(&self) -> Option<PathBuf> {
        let home = dirs::home_dir()?;
        Some(match self {
            Shell::Bash => home.join(".bashrc"),
            Shell::Zsh => home.join(".zshrc"),
            Shell::Fish => home.join(".config/fish/config.fish"),
        })
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Shell::Bash => "bash",
            Shell::Zsh => "zsh",
            Shell::Fish => "fish",
        }
    }
}

/// Render the shell-hook snippet a user can paste into their shell rc. This
/// does not alias or wrap the AI command. It only records terminal state before
/// a known AI CLI starts and clears it when the prompt returns, so `watch` can
/// identify terminal targets without depending on emulator title heuristics.
pub fn render_shell_hook_snippet(shell: Shell, binaries: &[&str], beforepaste_exe: &str) -> String {
    render_shell_hook_snippet_with_title_mode(shell, binaries, beforepaste_exe, TitleMode::Preserve)
}

pub fn render_shell_hook_snippet_with_title_mode(
    shell: Shell,
    binaries: &[&str],
    beforepaste_exe: &str,
    title_mode: TitleMode,
) -> String {
    match shell {
        Shell::Zsh => render_zsh_hook(binaries, beforepaste_exe, title_mode),
        Shell::Bash => render_bash_hook(binaries, beforepaste_exe, title_mode),
        Shell::Fish => render_fish_hook(binaries, beforepaste_exe, title_mode),
    }
}

fn render_zsh_hook(binaries: &[&str], beforepaste_exe: &str, title_mode: TitleMode) -> String {
    let bins = binaries
        .iter()
        .map(|b| shell_single_quote(b))
        .collect::<Vec<_>>()
        .join(" ");
    let exe = shell_single_quote(beforepaste_exe);
    let (title_setup, title_clear, title_start) = match title_mode {
        TitleMode::Preserve => ("", "", ""),
        TitleMode::Keepalive => (
            "typeset -g __beforepaste_title_pid=\"\"\n\
             _beforepaste_title_project() {\n\
             \tprintf '\\033]0;%s\\007' \"${PWD:t}\"\n\
             }\n\
             _beforepaste_stop_title_keepalive() {\n\
             \tif [[ -n \"$__beforepaste_title_pid\" ]]; then\n\
             \t\tkill \"$__beforepaste_title_pid\" >/dev/null 2>&1 || true\n\
             \t\t__beforepaste_title_pid=\"\"\n\
             \tfi\n\
             }\n\
             _beforepaste_start_title_keepalive() {\n\
             \tlocal _ps_title=\"beforepaste:$__beforepaste_ai_current:${PWD:t}\"\n\
             \tprintf '\\033]0;%s\\007' \"$_ps_title\"\n\
             \t{ while true; do printf '\\033]0;%s\\007' \"$_ps_title\" > /dev/tty; sleep 0.5; done } &\n\
             \t__beforepaste_title_pid=$!\n\
             \tdisown \"$__beforepaste_title_pid\" >/dev/null 2>&1 || true\n\
             }\n",
            "\t_beforepaste_stop_title_keepalive\n\
             \t_beforepaste_title_project\n",
            "\t_beforepaste_start_title_keepalive\n",
        ),
    };
    format!(
         "{BLOCK_BEGIN}\n\
         typeset -ga __beforepaste_ai_bins=({bins})\n\
         typeset -g __beforepaste_ai_current=\"${{__beforepaste_ai_current:-}}\"\n\
         typeset -g __beforepaste_ai_cmdline=\"${{__beforepaste_ai_cmdline:-}}\"\n\
         typeset -g __beforepaste_terminal_app=\"${{__beforepaste_terminal_app:-}}\"\n\
         typeset -g __beforepaste_terminal_id=\"${{__beforepaste_terminal_id:-}}\"\n\
         {title_setup}\
         _beforepaste_detect_terminal_identity() {{\n\
         \t__beforepaste_terminal_app=\"\"\n\
         \t__beforepaste_terminal_id=\"\"\n\
         \tif [[ \"${{TERM_PROGRAM:-}}\" == \"ghostty\" && -x /usr/bin/osascript ]]; then\n\
         \t\tlocal _ps_terminal_id\n\
         \t\t_ps_terminal_id=$(/usr/bin/osascript -e 'tell application \"Ghostty\" to get id of focused terminal of selected tab of front window' 2>/dev/null) || _ps_terminal_id=\"\"\n\
         \t\tif [[ -n \"$_ps_terminal_id\" ]]; then\n\
         \t\t\t__beforepaste_terminal_app=\"ghostty\"\n\
         \t\t\t__beforepaste_terminal_id=\"$_ps_terminal_id\"\n\
         \t\tfi\n\
         \tfi\n\
         }}\n\
         _beforepaste_write_target() {{\n\
         \tlocal -a _ps_identity_args\n\
         \t_ps_identity_args=()\n\
         \tif [[ -z \"$__beforepaste_terminal_app\" || -z \"$__beforepaste_terminal_id\" ]]; then\n\
         \t\t_beforepaste_detect_terminal_identity\n\
         \tfi\n\
         \tif [[ -n \"$__beforepaste_terminal_app\" && -n \"$__beforepaste_terminal_id\" ]]; then\n\
         \t\t_ps_identity_args=(--terminal-app \"$__beforepaste_terminal_app\" --terminal-id \"$__beforepaste_terminal_id\")\n\
         \tfi\n\
         \tcommand {exe} terminal-enter --cmd \"$__beforepaste_ai_cmdline\" --cwd \"$PWD\" --tty \"$(tty)\" \"${{_ps_identity_args[@]}}\" --ttl-secs {STATE_TTL_SECS} >/dev/null 2>&1 || true\n\
         }}\n\
         _beforepaste_clear_target() {{\n\
         \tcommand {exe} terminal-leave --tty \"$(tty)\" >/dev/null 2>&1 || true\n\
         \t__beforepaste_terminal_app=\"\"\n\
         \t__beforepaste_terminal_id=\"\"\n\
         {title_clear}\
         }}\n\
         _beforepaste_is_ai_cmd() {{\n\
         \t__beforepaste_ai_current=\"\"\n\
         \tlocal -a _ps_words\n\
         \t_ps_words=(\"${{(z)1}}\")\n\
         \tlocal _ps_cmd\n\
         \tlocal _ps_bin\n\
         \tfor _ps_cmd in \"${{_ps_words[@]}}\"; do\n\
         \t\tcase \"$_ps_cmd\" in\n\
         \t\t\t*=*|command|builtin|exec|noglob|env|sudo|-*) continue ;;\n\
         \t\tesac\n\
         \t\t_ps_cmd=\"${{_ps_cmd:t}}\"\n\
         \t\tfor _ps_bin in \"${{__beforepaste_ai_bins[@]}}\"; do\n\
         \t\t\tif [[ \"$_ps_cmd\" == \"$_ps_bin\" ]]; then\n\
         \t\t\t\t__beforepaste_ai_current=\"$_ps_bin\"\n\
         \t\t\t\treturn 0\n\
         \t\t\tfi\n\
         \t\tdone\n\
         \t\treturn 1\n\
         \tdone\n\
         \treturn 1\n\
         }}\n\
         _beforepaste_preexec() {{\n\
         \tlocal _ps_cmdline=\"${{3:-$1}}\"\n\
         \t_beforepaste_clear_target\n\
         \t_beforepaste_is_ai_cmd \"$_ps_cmdline\" || return 0\n\
         \t__beforepaste_ai_cmdline=\"$_ps_cmdline\"\n\
         \t_beforepaste_detect_terminal_identity\n\
         \t_beforepaste_write_target\n\
         {title_start}\
         }}\n\
         _beforepaste_precmd() {{\n\
         \t_beforepaste_clear_target\n\
         }}\n\
         autoload -Uz add-zsh-hook\n\
         add-zsh-hook -d preexec _beforepaste_preexec >/dev/null 2>&1 || true\n\
         add-zsh-hook -d precmd _beforepaste_precmd >/dev/null 2>&1 || true\n\
         add-zsh-hook preexec _beforepaste_preexec\n\
         add-zsh-hook precmd _beforepaste_precmd\n\
         {BLOCK_END}\n"
    )
}

fn render_bash_hook(binaries: &[&str], beforepaste_exe: &str, title_mode: TitleMode) -> String {
    let bins = binaries
        .iter()
        .map(|b| shell_single_quote(b))
        .collect::<Vec<_>>()
        .join(" ");
    let exe = shell_single_quote(beforepaste_exe);
    let (title_setup, title_clear, title_start) = match title_mode {
        TitleMode::Preserve => ("", "", ""),
        TitleMode::Keepalive => (
            "__beforepaste_title_pid=\"\"\n\
             __beforepaste_title_project() {\n\
             \tprintf '\\033]0;%s\\007' \"$(basename -- \"$PWD\")\"\n\
             }\n\
             __beforepaste_stop_title_keepalive() {\n\
             \tif [[ -n \"$__beforepaste_title_pid\" ]]; then\n\
             \t\tkill \"$__beforepaste_title_pid\" >/dev/null 2>&1 || true\n\
             \t\t__beforepaste_title_pid=\"\"\n\
             \tfi\n\
             }\n\
             __beforepaste_start_title_keepalive() {\n\
             \tlocal _ps_title=\"beforepaste:$__beforepaste_ai_current:$(basename -- \"$PWD\")\"\n\
             \tprintf '\\033]0;%s\\007' \"$_ps_title\"\n\
             \twhile true; do printf '\\033]0;%s\\007' \"$_ps_title\" > /dev/tty; sleep 0.5; done &\n\
             \t__beforepaste_title_pid=$!\n\
             \tdisown \"$__beforepaste_title_pid\" >/dev/null 2>&1 || true\n\
             }\n",
            "\t__beforepaste_stop_title_keepalive\n\
             \t__beforepaste_title_project\n",
            "\t\t__beforepaste_start_title_keepalive\n",
        ),
    };
    format!(
         "{BLOCK_BEGIN}\n\
         __beforepaste_ai_bins=({bins})\n\
         __beforepaste_ai_current=\"\"\n\
         __beforepaste_ai_cmdline=\"\"\n\
         __beforepaste_terminal_app=\"\"\n\
         __beforepaste_terminal_id=\"\"\n\
         __beforepaste_in_hook=0\n\
         {title_setup}\
         __beforepaste_detect_terminal_identity() {{\n\
         \t__beforepaste_terminal_app=\"\"\n\
         \t__beforepaste_terminal_id=\"\"\n\
         \tif [[ \"${{TERM_PROGRAM:-}}\" == \"ghostty\" && -x /usr/bin/osascript ]]; then\n\
         \t\tlocal _ps_terminal_id\n\
         \t\t_ps_terminal_id=$(/usr/bin/osascript -e 'tell application \"Ghostty\" to get id of focused terminal of selected tab of front window' 2>/dev/null) || _ps_terminal_id=\"\"\n\
         \t\tif [[ -n \"$_ps_terminal_id\" ]]; then\n\
         \t\t\t__beforepaste_terminal_app=\"ghostty\"\n\
         \t\t\t__beforepaste_terminal_id=\"$_ps_terminal_id\"\n\
         \t\tfi\n\
         \tfi\n\
         }}\n\
         __beforepaste_write_target() {{\n\
         \tlocal _ps_identity_args=()\n\
         \tif [[ -z \"$__beforepaste_terminal_app\" || -z \"$__beforepaste_terminal_id\" ]]; then\n\
         \t\t__beforepaste_detect_terminal_identity\n\
         \tfi\n\
         \tif [[ -n \"$__beforepaste_terminal_app\" && -n \"$__beforepaste_terminal_id\" ]]; then\n\
         \t\t_ps_identity_args=(--terminal-app \"$__beforepaste_terminal_app\" --terminal-id \"$__beforepaste_terminal_id\")\n\
         \tfi\n\
         \tcommand {exe} terminal-enter --cmd \"$__beforepaste_ai_cmdline\" --cwd \"$PWD\" --tty \"$(tty)\" \"${{_ps_identity_args[@]}}\" --ttl-secs {STATE_TTL_SECS} >/dev/null 2>&1 || true\n\
         }}\n\
         __beforepaste_clear_target() {{\n\
         \tcommand {exe} terminal-leave --tty \"$(tty)\" >/dev/null 2>&1 || true\n\
         \t__beforepaste_terminal_app=\"\"\n\
         \t__beforepaste_terminal_id=\"\"\n\
         {title_clear}\
         }}\n\
         __beforepaste_is_ai_cmd() {{\n\
         \t__beforepaste_ai_current=\"\"\n\
         \tlocal _ps_cmd\n\
         \tlocal _ps_bin\n\
         \tfor _ps_cmd in $1; do\n\
         \t\tcase \"$_ps_cmd\" in\n\
         \t\t\t*=*|command|builtin|exec|noglob|env|sudo|-*) continue ;;\n\
         \t\tesac\n\
         \t\t_ps_cmd=\"${{_ps_cmd##*/}}\"\n\
         \t\tfor _ps_bin in \"${{__beforepaste_ai_bins[@]}}\"; do\n\
         \t\t\tif [[ \"$_ps_cmd\" == \"$_ps_bin\" ]]; then\n\
         \t\t\t\t__beforepaste_ai_current=\"$_ps_bin\"\n\
         \t\t\t\treturn 0\n\
         \t\t\tfi\n\
         \t\tdone\n\
         \t\treturn 1\n\
         \tdone\n\
         \treturn 1\n\
         }}\n\
         __beforepaste_debug_trap() {{\n\
         \t[[ \"$__beforepaste_in_hook\" == 1 ]] && return 0\n\
         \t__beforepaste_in_hook=1\n\
         \t__beforepaste_clear_target\n\
         \tif __beforepaste_is_ai_cmd \"$BASH_COMMAND\"; then\n\
         \t\t__beforepaste_ai_cmdline=\"$BASH_COMMAND\"\n\
         \t\t__beforepaste_detect_terminal_identity\n\
         \t\t__beforepaste_write_target\n\
         {title_start}\
         \tfi\n\
         \t__beforepaste_in_hook=0\n\
         }}\n\
         __beforepaste_prompt_command() {{\n\
         \t__beforepaste_clear_target\n\
         }}\n\
         trap '__beforepaste_debug_trap' DEBUG\n\
         PROMPT_COMMAND=\"__beforepaste_prompt_command${{PROMPT_COMMAND:+;$PROMPT_COMMAND}}\"\n\
         {BLOCK_END}\n"
    )
}

fn render_fish_hook(binaries: &[&str], beforepaste_exe: &str, title_mode: TitleMode) -> String {
    let bins = binaries
        .iter()
        .map(|b| shell_single_quote(b))
        .collect::<Vec<_>>()
        .join(" ");
    let exe = shell_single_quote(beforepaste_exe);
    let (title_setup, title_clear, title_start) = match title_mode {
        TitleMode::Preserve => ("", "", ""),
        TitleMode::Keepalive => (
            "set -g __beforepaste_title_pid \"\"\n\
             function __beforepaste_title_project\n\
             \tprintf '\\033]0;%s\\007' (basename -- \"$PWD\")\n\
             end\n\
             function __beforepaste_stop_title_keepalive\n\
             \tif test -n \"$__beforepaste_title_pid\"\n\
             \t\tkill $__beforepaste_title_pid >/dev/null 2>&1\n\
             \t\tset -g __beforepaste_title_pid \"\"\n\
             \tend\n\
             end\n\
             function __beforepaste_start_title_keepalive\n\
             \tset -l _ps_title \"beforepaste:$__beforepaste_ai_current:\"(basename -- \"$PWD\")\n\
             \tprintf '\\033]0;%s\\007' $_ps_title\n\
             \twhile true; printf '\\033]0;%s\\007' $_ps_title > /dev/tty; sleep 0.5; end &\n\
             \tset -g __beforepaste_title_pid $last_pid\n\
             \tdisown $__beforepaste_title_pid >/dev/null 2>&1\n\
             end\n",
            "\t__beforepaste_stop_title_keepalive\n\
             \t__beforepaste_title_project\n",
            "\t\t\t__beforepaste_start_title_keepalive\n",
        ),
    };
    format!(
         "{BLOCK_BEGIN}\n\
         set -g __beforepaste_ai_bins {bins}\n\
         set -g __beforepaste_ai_current \"\"\n\
         set -g __beforepaste_ai_cmdline \"\"\n\
         set -g __beforepaste_terminal_app \"\"\n\
         set -g __beforepaste_terminal_id \"\"\n\
         {title_setup}\
         function __beforepaste_detect_terminal_identity\n\
         \tset -g __beforepaste_terminal_app \"\"\n\
         \tset -g __beforepaste_terminal_id \"\"\n\
         \tif set -q TERM_PROGRAM; and test \"$TERM_PROGRAM\" = ghostty; and test -x /usr/bin/osascript\n\
         \t\tset -l _ps_terminal_id (/usr/bin/osascript -e 'tell application \"Ghostty\" to get id of focused terminal of selected tab of front window' 2>/dev/null)\n\
         \t\tif test -n \"$_ps_terminal_id\"\n\
         \t\t\tset -g __beforepaste_terminal_app ghostty\n\
         \t\t\tset -g __beforepaste_terminal_id \"$_ps_terminal_id\"\n\
         \t\tend\n\
         \tend\n\
         end\n\
         function __beforepaste_write_target\n\
         \tset -l _ps_identity_args\n\
         \tif test -z \"$__beforepaste_terminal_app\"; or test -z \"$__beforepaste_terminal_id\"\n\
         \t\t__beforepaste_detect_terminal_identity\n\
         \tend\n\
         \tif test -n \"$__beforepaste_terminal_app\"; and test -n \"$__beforepaste_terminal_id\"\n\
         \t\tset _ps_identity_args --terminal-app \"$__beforepaste_terminal_app\" --terminal-id \"$__beforepaste_terminal_id\"\n\
         \tend\n\
         \tcommand {exe} terminal-enter --cmd \"$__beforepaste_ai_cmdline\" --cwd \"$PWD\" --tty (tty) $_ps_identity_args --ttl-secs {STATE_TTL_SECS} >/dev/null 2>&1\n\
         end\n\
         function __beforepaste_clear_target\n\
         \tcommand {exe} terminal-leave --tty (tty) >/dev/null 2>&1\n\
         \tset -g __beforepaste_terminal_app \"\"\n\
         \tset -g __beforepaste_terminal_id \"\"\n\
         {title_clear}\
         end\n\
         function __beforepaste_preexec --on-event fish_preexec\n\
         \tset -g __beforepaste_ai_current \"\"\n\
         \t__beforepaste_clear_target\n\
         \tfor _ps_cmd in (string split ' ' -- $argv[1])\n\
         \t\tswitch $_ps_cmd\n\
         \t\t\tcase '*=*' command builtin exec noglob env sudo '-*'\n\
         \t\t\t\tcontinue\n\
         \t\tend\n\
         \t\tset _ps_cmd (basename -- $_ps_cmd)\n\
         \t\tif contains -- $_ps_cmd $__beforepaste_ai_bins\n\
         \t\t\tset -g __beforepaste_ai_current $_ps_cmd\n\
         \t\t\tset -g __beforepaste_ai_cmdline \"$argv[1]\"\n\
         \t\t\t__beforepaste_detect_terminal_identity\n\
         \t\t\t__beforepaste_write_target\n\
         {title_start}\
         \t\tend\n\
         \t\tbreak\n\
         \tend\n\
         end\n\
         function __beforepaste_postexec --on-event fish_postexec\n\
         \t__beforepaste_clear_target\n\
         end\n\
         {BLOCK_END}\n"
    )
}

fn shell_single_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Remove a previously-pasted BeforePaste shell integration block from `rc_path`.
///
/// Returns `Ok(true)` if a block was found and removed, `Ok(false)` if no
/// block was present (file missing, empty, or no BeforePaste markers).
/// Atomic write via tempfile + rename so a torn rc is impossible.
///
/// Honors `BEFOREPASTE_NO_OS_SIDE_EFFECTS`: returns `Ok(false)` without
/// touching the file when set, so tests can exercise `run_uninstall`
/// without writing to the developer's real shell rc.
///
/// Detection strategy, in order:
/// 1. Two adjacent lines both equal to `BLOCK_BEGIN` (the labelled fence) -
///    everything between them is removed, fences included. Legacy paste-guard
///    fences are also removed.
/// 2. Fallback: any line containing `ALIAS_MARKER` or `HOOK_MARKER` is removed
///    individually. Catches the case where the user kept the managed line but
///    deleted the fence comments.
pub fn uninstall_aliases(rc_path: &Path) -> anyhow::Result<bool> {
    if !os_side_effects_allowed() {
        return Ok(false);
    }
    let existing = match std::fs::read_to_string(rc_path) {
        Ok(s) => s,
        Err(_) => return Ok(false),
    };
    let (new_contents, changed) = strip_paste_guard_block(&existing);
    if !changed {
        return Ok(false);
    }
    atomic_write(rc_path, new_contents.as_bytes())?;
    Ok(true)
}

/// Pure string transform behind `uninstall_aliases`. Lets the tests exercise
/// the parser without hitting the filesystem.
pub fn strip_paste_guard_block(contents: &str) -> (String, bool) {
    let lines: Vec<&str> = contents.split_inclusive('\n').collect();
    let mut out: Vec<&str> = Vec::with_capacity(lines.len());
    let mut changed = false;
    let mut i = 0;
    while i < lines.len() {
        let trimmed = lines[i].trim_end();
        if trimmed == BLOCK_BEGIN || trimmed == LEGACY_BLOCK_BEGIN {
            let fence_end = if trimmed == BLOCK_BEGIN {
                BLOCK_END
            } else {
                LEGACY_BLOCK_END
            };
            // Find the matching closing fence at the same trimmed value.
            let mut j = i + 1;
            while j < lines.len() && lines[j].trim_end() != fence_end {
                j += 1;
            }
            if j < lines.len() {
                // Skip lines [i, j] inclusive.
                changed = true;
                i = j + 1;
                continue;
            }
            // Unterminated fence - leave the file alone rather than chew
            // an arbitrary tail; user can clean by hand.
        }
        if lines[i].contains(ALIAS_MARKER) || lines[i].contains(HOOK_MARKER) {
            changed = true;
            i += 1;
            continue;
        }
        out.push(lines[i]);
        i += 1;
    }
    // Collapse a run of consecutive blank lines that the removal may have
    // produced. Conservative: keep at most one blank between non-blank
    // lines.
    let joined: String = out.into_iter().collect();
    let collapsed = collapse_blank_runs(&joined);
    (collapsed, changed)
}

fn collapse_blank_runs(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut blank_run = 0;
    for line in s.split_inclusive('\n') {
        if line.trim().is_empty() {
            blank_run += 1;
            if blank_run <= 1 {
                out.push_str(line);
            }
        } else {
            blank_run = 0;
            out.push_str(line);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippet_bash_format() {
        let s = render_shell_hook_snippet(Shell::Bash, &["claude", "codex"], "/tmp/beforepaste");
        assert!(s.contains("__beforepaste_ai_bins=('claude' 'codex')"));
        assert!(s.contains("command '/tmp/beforepaste' terminal-enter"));
        assert!(s.contains("command '/tmp/beforepaste' terminal-leave"));
        assert!(!s.contains("__beforepaste_start_title_keepalive"));
        assert!(!s.contains("sleep 0.5"));
        assert!(!s.contains("alias codex"));
        assert!(s.contains(BLOCK_BEGIN));
        assert!(s.contains(BLOCK_END));
    }

    #[test]
    fn snippet_zsh_uses_add_zsh_hook() {
        let s = render_shell_hook_snippet(Shell::Zsh, &["claude", "codex"], "/tmp/beforepaste");
        assert!(s.contains("add-zsh-hook preexec _beforepaste_preexec"));
        assert!(s.contains("add-zsh-hook precmd _beforepaste_precmd"));
        assert!(s.contains("command '/tmp/beforepaste' terminal-enter"));
        assert!(!s.contains("_beforepaste_start_title_keepalive"));
        assert!(!s.contains("_beforepaste_stop_title_keepalive"));
        assert!(!s.contains("alias codex"));
    }

    #[test]
    fn snippets_do_not_start_state_heartbeat_jobs() {
        for shell in [Shell::Bash, Shell::Zsh, Shell::Fish] {
            let s = render_shell_hook_snippet(shell, &["claude", "codex"], "/tmp/beforepaste");
            assert!(!s.contains("state_heartbeat"));
            assert!(!s.contains("__beforepaste_state_pid"));
            assert!(!s.contains("while true; do sleep 15"));
            assert!(!s.contains("while true; sleep 15"));
        }
    }

    #[test]
    fn snippet_fish_format_differs() {
        let s = render_shell_hook_snippet(Shell::Fish, &["claude"], "/tmp/beforepaste");
        assert!(s.contains("function __beforepaste_preexec --on-event fish_preexec"));
        assert!(s.contains("set -g __beforepaste_ai_bins 'claude'"));
        assert!(!s.contains("function __beforepaste_start_title_keepalive"));
        assert!(!s.contains("function __beforepaste_stop_title_keepalive"));
        assert!(!s.contains("alias claude"));
    }

    #[test]
    fn keepalive_title_mode_writes_beforepaste_marker() {
        let s = render_shell_hook_snippet_with_title_mode(
            Shell::Zsh,
            &["codex"],
            "/tmp/beforepaste",
            TitleMode::Keepalive,
        );
        assert!(s.contains("_beforepaste_start_title_keepalive"));
        assert!(s.contains("beforepaste:$__beforepaste_ai_current"));
        assert!(s.contains("sleep 0.5"));
    }

    #[test]
    fn snippet_quotes_executable_path() {
        let s = render_shell_hook_snippet(
            Shell::Zsh,
            &["codex"],
            "/Applications/BeforePaste/bin/beforepaste",
        );
        assert!(s.contains("command '/Applications/BeforePaste/bin/beforepaste' terminal-leave"));
    }

    #[test]
    fn display_name_round_trip() {
        assert_eq!(Shell::Bash.display_name(), "bash");
        assert_eq!(Shell::Zsh.display_name(), "zsh");
        assert_eq!(Shell::Fish.display_name(), "fish");
    }

    #[test]
    fn strip_removes_fenced_block() {
        let input = "export FOO=1\n\
                     # ---- beforepaste shell integration ----\n\
                     command beforepaste terminal-enter --cmd \"$1\" --cwd \"$PWD\" --tty \"$(tty)\"\n\
                     # ---- beforepaste shell integration ----\n\
                     export BAR=2\n";
        let (out, changed) = strip_paste_guard_block(input);
        assert!(changed);
        assert!(!out.contains("beforepaste terminal-enter"));
        assert!(out.contains("export FOO=1"));
        assert!(out.contains("export BAR=2"));
    }

    #[test]
    fn strip_removes_legacy_fenced_block() {
        let input = "export FOO=1\n\
                     # ---- beforepaste paste-guard ----\n\
                     alias claude='beforepaste paste-guard -- claude'\n\
                     # ---- beforepaste paste-guard ----\n\
                     export BAR=2\n";
        let (out, changed) = strip_paste_guard_block(input);
        assert!(changed);
        assert!(!out.contains("beforepaste paste-guard"));
        assert!(out.contains("export FOO=1"));
        assert!(out.contains("export BAR=2"));
    }

    #[test]
    fn strip_removes_orphan_alias_without_fence() {
        let input = "export FOO=1\n\
                     alias claude='beforepaste paste-guard -- claude'\n\
                     export BAR=2\n";
        let (out, changed) = strip_paste_guard_block(input);
        assert!(changed);
        assert!(!out.contains("beforepaste paste-guard"));
        assert!(out.contains("export FOO=1"));
        assert!(out.contains("export BAR=2"));
    }

    #[test]
    fn strip_removes_orphan_hook_without_fence() {
        let input = "export FOO=1\n\
                     command beforepaste terminal-enter --cmd \"$1\" --cwd \"$PWD\" --tty \"$(tty)\"\n\
                     export BAR=2\n";
        let (out, changed) = strip_paste_guard_block(input);
        assert!(changed);
        assert!(!out.contains("beforepaste terminal-enter"));
        assert!(out.contains("export FOO=1"));
        assert!(out.contains("export BAR=2"));
    }

    #[test]
    fn strip_no_op_when_clean() {
        let input = "export FOO=1\nexport BAR=2\n";
        let (out, changed) = strip_paste_guard_block(input);
        assert!(!changed);
        assert_eq!(out, input);
    }

    #[test]
    fn strip_collapses_blank_runs() {
        let input = "export FOO=1\n\n\n\n# ---- beforepaste shell integration ----\n\
                     command beforepaste terminal-enter --cmd \"$1\" --cwd \"$PWD\" --tty \"$(tty)\"\n\
                     # ---- beforepaste shell integration ----\n\n\n\nexport BAR=2\n";
        let (out, changed) = strip_paste_guard_block(input);
        assert!(changed);
        // No more than one consecutive blank line remains.
        assert!(!out.contains("\n\n\n"));
    }

    #[test]
    #[serial_test::serial(shell_env)]
    fn uninstall_aliases_round_trip_with_real_file() {
        let saved = std::env::var_os(ENV_NO_OS);
        std::env::remove_var(ENV_NO_OS);

        let dir = tempfile::tempdir().unwrap();
        let rc = dir.path().join(".bashrc");
        let original = "export FOO=1\nexport BAR=2\n";
        let with_block = format!(
            "{}# ---- beforepaste shell integration ----\n\
             command beforepaste terminal-enter --cmd \"$1\" --cwd \"$PWD\" --tty \"$(tty)\"\n\
             # ---- beforepaste shell integration ----\n",
            original
        );
        std::fs::write(&rc, &with_block).unwrap();

        let removed = uninstall_aliases(&rc).unwrap();
        assert!(removed);
        let after = std::fs::read_to_string(&rc).unwrap();
        assert!(!after.contains("beforepaste terminal-enter"));
        assert!(after.contains("export FOO=1"));

        if let Some(v) = saved {
            std::env::set_var(ENV_NO_OS, v);
        }
    }

    #[test]
    #[serial_test::serial(shell_env)]
    fn uninstall_no_op_when_env_set() {
        let dir = tempfile::tempdir().unwrap();
        let rc = dir.path().join(".bashrc");
        std::fs::write(&rc, "alias x='beforepaste paste-guard -- x'\n").unwrap();
        std::env::set_var(ENV_NO_OS, "1");
        let removed = uninstall_aliases(&rc).unwrap();
        std::env::remove_var(ENV_NO_OS);
        assert!(!removed);
        // File untouched.
        assert!(std::fs::read_to_string(&rc)
            .unwrap()
            .contains("beforepaste paste-guard"));
    }
}
