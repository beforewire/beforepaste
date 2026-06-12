pub const AI_CLI_BINARIES: &[&str] =
    &["codex", "gemini", "claude", "aider", "continue", "opencode"];

pub fn classify_command_line(command_line: &str) -> Option<&'static str> {
    let words = tokenize(command_line);
    classify_tokens(&words)
}

pub fn classify_binary_name(name: &str) -> Option<&'static str> {
    let name = name.rsplit('/').next().unwrap_or(name);
    let normalized = normalize_binary_name(name)?;
    AI_CLI_BINARIES.iter().copied().find(|kind| {
        normalized == *kind
            || normalized
                .strip_prefix(*kind)
                .is_some_and(|rest| rest.starts_with('-') || rest.starts_with('_'))
    })
}

fn tokenize(command_line: &str) -> Vec<String> {
    command_line
        .split_whitespace()
        .map(clean_word)
        .filter(|word| !word.is_empty())
        .collect()
}

fn clean_word(word: &str) -> String {
    word.trim_matches(|c: char| matches!(c, '\'' | '"' | '`'))
        .to_string()
}

fn classify_tokens(words: &[String]) -> Option<&'static str> {
    let mut i = 0;
    while i < words.len() {
        let word = words[i].as_str();
        if should_skip_prefix_word(word) {
            i += 1;
            continue;
        }

        let command = basename(word);
        if let Some(kind) = classify_binary_name(command) {
            return Some(kind);
        }
        if is_shell_wrapper(command) {
            return classify_shell_invocation(words, i + 1);
        }
        if is_package_runner(command) {
            return classify_package_runner(command, words, i + 1);
        }
        if is_interpreter(command) {
            return classify_interpreter_args(words, i + 1);
        }

        return None;
    }
    None
}

fn basename(word: &str) -> &str {
    word.rsplit('/').next().unwrap_or(word)
}

fn normalize_binary_name(name: &str) -> Option<String> {
    let mut normalized = name
        .trim()
        .trim_end_matches(".exe")
        .trim_end_matches(".cmd")
        .trim_end_matches(".bat")
        .trim_end_matches(".ps1")
        .trim_end_matches(".mjs")
        .trim_end_matches(".cjs")
        .trim_end_matches(".js")
        .trim_end_matches(".ts")
        .to_ascii_lowercase();
    if normalized.ends_with('.') {
        normalized.pop();
    }
    if normalized.is_empty() || normalized.contains('.') {
        return None;
    }
    Some(normalized)
}

fn should_skip_prefix_word(word: &str) -> bool {
    word.contains('=')
        || matches!(
            basename(word),
            "command" | "builtin" | "exec" | "noglob" | "env" | "sudo"
        )
        || word.starts_with('-')
}

fn is_shell_wrapper(command: &str) -> bool {
    matches!(command, "sh" | "bash" | "zsh" | "fish")
}

fn classify_shell_invocation(words: &[String], start: usize) -> Option<&'static str> {
    let mut i = start;
    while i < words.len() {
        let word = words[i].as_str();
        if shell_flag_runs_command(word) {
            let command = words.get(i + 1..)?.join(" ");
            return classify_command_line(&command);
        }
        if word.starts_with('-') {
            i += 1;
            continue;
        }
        return None;
    }
    None
}

fn shell_flag_runs_command(word: &str) -> bool {
    word.starts_with('-') && word.ends_with('c')
}

fn is_package_runner(command: &str) -> bool {
    matches!(
        command,
        "npx" | "pnpx" | "bunx" | "uvx" | "npm" | "pnpm" | "yarn" | "bun"
    )
}

fn classify_package_runner(runner: &str, words: &[String], start: usize) -> Option<&'static str> {
    if matches!(runner, "npx" | "pnpx" | "bunx" | "uvx") {
        return classify_runner_command(words, start);
    }

    let mut i = start;
    while i < words.len() {
        let word = words[i].as_str();
        if runner_option_takes_value(word) {
            i += 2;
            continue;
        }
        if word.starts_with('-') || word.contains('=') {
            i += 1;
            continue;
        }
        let subcommand = basename(word);
        if matches!(subcommand, "exec" | "dlx" | "x") {
            return classify_runner_command(words, i + 1);
        }
        return None;
    }
    None
}

fn classify_runner_command(words: &[String], start: usize) -> Option<&'static str> {
    let mut i = start;
    while i < words.len() {
        let word = words[i].as_str();
        if runner_option_takes_value(word) {
            i += 2;
            continue;
        }
        if word.starts_with('-') || word.contains('=') {
            i += 1;
            continue;
        }
        return classify_binary_name(word).or_else(|| classify_node_modules_path(word));
    }
    None
}

fn classify_node_modules_path(path: &str) -> Option<&'static str> {
    if !path.contains("node_modules") {
        return None;
    }
    let mut after_marker = false;
    for component in path.split('/') {
        if component == "node_modules" {
            after_marker = true;
            continue;
        }
        if !after_marker || component.starts_with('@') {
            continue;
        }
        if let Some(kind) = classify_binary_name(component) {
            return Some(kind);
        }
    }
    None
}

fn runner_option_takes_value(word: &str) -> bool {
    matches!(
        word,
        "-p" | "--package" | "-C" | "--dir" | "--cwd" | "--prefix" | "--registry"
    )
}

fn is_interpreter(command: &str) -> bool {
    matches!(
        command,
        "node" | "deno" | "python" | "python3" | "uv" | "tsx" | "ts-node"
    )
}

fn classify_interpreter_args(words: &[String], start: usize) -> Option<&'static str> {
    let mut i = start;
    while i < words.len() {
        let word = words[i].as_str();
        if word == "-m" {
            return words
                .get(i + 1)
                .and_then(|module| classify_binary_name(module));
        }
        if word.starts_with('-') || word.contains('=') {
            i += 1;
            continue;
        }
        return classify_binary_name(word).or_else(|| classify_node_modules_path(word));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_direct_commands() {
        assert_eq!(classify_command_line("codex resume abc"), Some("codex"));
        assert_eq!(classify_command_line("/opt/bin/gemini"), Some("gemini"));
        assert_eq!(
            classify_command_line("env ANTHROPIC_API_KEY=x claude"),
            Some("claude")
        );
        assert_eq!(classify_command_line("continue"), Some("continue"));
    }

    #[test]
    fn classifies_common_wrappers() {
        assert_eq!(classify_command_line("npx -y @openai/codex"), Some("codex"));
        assert_eq!(classify_command_line("pnpm dlx gemini-cli"), Some("gemini"));
        assert_eq!(
            classify_command_line("node /opt/lib/node_modules/@anthropic-ai/claude-code/cli.js"),
            Some("claude")
        );
        assert_eq!(classify_command_line("python -m aider"), Some("aider"));
        assert_eq!(
            classify_command_line("zsh -lc 'opencode run'"),
            Some("opencode")
        );
    }

    #[test]
    fn avoids_plain_argument_false_positives() {
        assert_eq!(classify_command_line("vim .env"), None);
        assert_eq!(classify_command_line("vim codex-notes.md"), None);
        assert_eq!(classify_command_line("cat codex"), None);
        assert_eq!(classify_command_line("npm run test codex"), None);
        assert_eq!(classify_command_line("sh -c 'echo codex'"), None);
    }
}
