#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VscodeSurface {
    Terminal,
    Editor,
    AiView(String),
    Other,
    Unknown,
}

impl VscodeSurface {
    pub fn as_debug_label(&self) -> String {
        match self {
            Self::Terminal => "terminal".to_string(),
            Self::Editor => "editor".to_string(),
            Self::AiView(kind) => format!("ai-view:{kind}"),
            Self::Other => "other".to_string(),
            Self::Unknown => "unknown".to_string(),
        }
    }
}

pub fn classify_focus_probe(probe: &str) -> VscodeSurface {
    let normalized = probe.trim().to_ascii_lowercase();
    if normalized.is_empty() || normalized == "missing" {
        return VscodeSurface::Unknown;
    }

    if let Some(kind) = classify_ai_view_probe(&normalized) {
        return VscodeSurface::AiView(kind.to_string());
    }

    if normalized.contains("xterm-helper-textarea")
        || normalized.contains("xterm-screen")
        || normalized.contains("terminal.integrated")
        || normalized.contains("terminal ")
    {
        return VscodeSurface::Terminal;
    }

    if normalized.contains("native-edit-context")
        || normalized.contains("monaco-mouse-cursor-text")
        || normalized.contains("the editor is not accessible")
    {
        return VscodeSurface::Editor;
    }

    VscodeSurface::Other
}

fn classify_ai_view_probe(normalized: &str) -> Option<&'static str> {
    for (marker, kind) in AI_VIEW_ID_MARKERS {
        if normalized.contains(marker) {
            return Some(kind);
        }
    }

    for line in normalized.lines() {
        let value = line
            .split_once('=')
            .map(|(_, value)| value)
            .unwrap_or(line)
            .trim();
        if value.is_empty() {
            continue;
        }
        for (title, kind) in AI_VIEW_TITLE_MARKERS {
            if value == *title
                || value.starts_with(&format!("{title} "))
                || value.starts_with(&format!("ask {title}"))
            {
                return Some(kind);
            }
        }
    }

    None
}

const AI_VIEW_ID_MARKERS: &[(&str, &str)] = &[
    ("chatgpt.sidebarview", "codex"),
    ("chatgpt.sidebarsecondaryview", "codex"),
    ("codexviewcontainer", "codex"),
    ("codexsecondaryviewcontainer", "codex"),
    ("openai.chatgpt", "codex"),
    ("claudevscodesidebar", "claude"),
    ("claudevscodesidebarsecondary", "claude"),
    ("anthropic.claude", "claude"),
];

const AI_VIEW_TITLE_MARKERS: &[(&str, &str)] = &[
    ("codex", "codex"),
    ("claude code", "claude"),
    ("gemini", "gemini"),
    ("continue", "continue"),
    ("aider", "aider"),
    ("opencode", "opencode"),
];

#[cfg(target_os = "macos")]
pub fn focused_surface() -> VscodeSurface {
    let Some(focused_probe) = osascript(VSCODE_FOCUSED_ELEMENT_PROBE, 600) else {
        return VscodeSurface::Unknown;
    };
    let focused_surface = classify_focus_probe(&focused_probe);
    if !matches!(
        focused_surface,
        VscodeSurface::Other | VscodeSurface::Unknown
    ) {
        return focused_surface;
    }

    let Some(ancestor_probe) = osascript(VSCODE_ANCESTOR_PROBE, 1_500) else {
        return focused_surface;
    };
    classify_focus_probe(&format!("{focused_probe}\n{ancestor_probe}"))
}

#[cfg(not(target_os = "macos"))]
pub fn focused_surface() -> VscodeSurface {
    VscodeSurface::Unknown
}

#[cfg(target_os = "macos")]
fn osascript(src: &str, timeout_ms: u64) -> Option<String> {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    let mut child = Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(src)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let start = Instant::now();
    loop {
        match child.try_wait().ok()? {
            Some(_) => break,
            None if start.elapsed() >= Duration::from_millis(timeout_ms) => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
            None => std::thread::sleep(Duration::from_millis(10)),
        }
    }

    let out = child.wait_with_output().ok()?;
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
const VSCODE_FOCUSED_ELEMENT_PROBE: &str = r#"tell application "System Events"
  tell (first application process whose frontmost is true)
    try
      set e to value of attribute "AXFocusedUIElement" of it
    on error
      return "missing"
    end try
    set roleValue to ""
    set descValue to ""
    set titleValue to ""
    set classValue to ""
    set identifierValue to ""
    set domIdentifierValue to ""
    try
      set roleValue to (value of attribute "AXRole" of e) as text
    end try
    try
      set descValue to (value of attribute "AXDescription" of e) as text
    end try
    try
      set titleValue to (value of attribute "AXTitle" of e) as text
    end try
    try
      set classValue to (value of attribute "AXDOMClassList" of e) as text
    end try
    try
      set identifierValue to (value of attribute "AXIdentifier" of e) as text
    end try
    try
      set domIdentifierValue to (value of attribute "AXDOMIdentifier" of e) as text
    end try
    return "role=" & roleValue & linefeed & "description=" & descValue & linefeed & "title=" & titleValue & linefeed & "class=" & classValue & linefeed & "identifier=" & identifierValue & linefeed & "dom_identifier=" & domIdentifierValue
  end tell
end tell"#;

#[cfg(target_os = "macos")]
const VSCODE_ANCESTOR_PROBE: &str = r#"tell application "System Events"
  tell (first application process whose frontmost is true)
    try
      set currentElement to value of attribute "AXFocusedUIElement" of it
    on error
      return "missing"
    end try
    set ancestryValue to ""
    repeat with i from 1 to 10
      set parentRole to ""
      set parentDesc to ""
      set parentTitle to ""
      set parentClass to ""
      set parentIdentifier to ""
      set parentDomIdentifier to ""
      try
        set parentRole to (value of attribute "AXRole" of currentElement) as text
      end try
      try
        set parentDesc to (value of attribute "AXDescription" of currentElement) as text
      end try
      try
        set parentTitle to (value of attribute "AXTitle" of currentElement) as text
      end try
      try
        set parentClass to (value of attribute "AXDOMClassList" of currentElement) as text
      end try
      try
        set parentIdentifier to (value of attribute "AXIdentifier" of currentElement) as text
      end try
      try
        set parentDomIdentifier to (value of attribute "AXDOMIdentifier" of currentElement) as text
      end try
      set ancestryValue to ancestryValue & linefeed & "ancestor_role=" & parentRole & linefeed & "ancestor_description=" & parentDesc & linefeed & "ancestor_title=" & parentTitle & linefeed & "ancestor_class=" & parentClass & linefeed & "ancestor_identifier=" & parentIdentifier & linefeed & "ancestor_dom_identifier=" & parentDomIdentifier
      try
        set currentElement to value of attribute "AXParent" of currentElement
      on error
        exit repeat
      end try
    end repeat
    return ancestryValue
  end tell
end tell"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_vscode_terminal_focus() {
        assert_eq!(
            classify_focus_probe(
                "AXTextField\nTerminal 1 environment is stale\n\nxterm-helper-textarea"
            ),
            VscodeSurface::Terminal
        );
    }

    #[test]
    fn classifies_vscode_editor_focus() {
        assert_eq!(
            classify_focus_probe(
                "AXTextArea\nThe editor is not accessible at this time\n\nnative-edit-context"
            ),
            VscodeSurface::Editor
        );
    }

    #[test]
    fn classifies_known_ai_views() {
        assert_eq!(
            classify_focus_probe("AXTextArea\nCodex prompt\n\nchatgpt.sidebarSecondaryView"),
            VscodeSurface::AiView("codex".to_string())
        );
        assert_eq!(
            classify_focus_probe("AXTextArea\nClaude Code input\n\n"),
            VscodeSurface::AiView("claude".to_string())
        );
    }

    #[test]
    fn classifies_ai_view_from_ancestor_title_before_editor_class() {
        assert_eq!(
            classify_focus_probe(
                "role=AXTextArea\n\
                 description=The editor is not accessible at this time\n\
                 title=\n\
                 class=native-edit-context\n\
                 ancestor_role=AXGroup\n\
                 ancestor_title=Codex"
            ),
            VscodeSurface::AiView("codex".to_string())
        );
    }

    #[test]
    fn codex_filename_does_not_mark_editor_as_ai_view() {
        assert_eq!(
            classify_focus_probe(
                "role=AXTextArea\n\
                 description=The editor is not accessible at this time\n\
                 title=codex-notes.md\n\
                 class=native-edit-context"
            ),
            VscodeSurface::Editor
        );
    }

    #[test]
    fn classifies_codex_view_dom_identifier() {
        assert_eq!(
            classify_focus_probe(
                "role=AXTextArea\n\
                 description=The editor is not accessible at this time\n\
                 class=native-edit-context\n\
                 ancestor_dom_identifier=chatgpt.sidebarSecondaryView"
            ),
            VscodeSurface::AiView("codex".to_string())
        );
    }

    #[test]
    fn classifies_codex_view_container_identifier() {
        assert_eq!(
            classify_focus_probe(
                "role=AXGroup\n\
                 identifier=codexSecondaryViewContainer\n\
                 title=Codex"
            ),
            VscodeSurface::AiView("codex".to_string())
        );
    }
}
