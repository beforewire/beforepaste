use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::process::Command;

use super::terminal_state;

#[derive(Debug, Clone)]
pub struct CleanupOptions {
    pub dry_run: bool,
    pub aggressive: bool,
}

#[derive(Debug, Clone, Default)]
pub struct CleanupReport {
    pub state_files: usize,
    pub removed_state_files: Vec<PathBuf>,
    pub legacy_active_state_files: Vec<PathBuf>,
    pub heartbeat_candidates: Vec<HeartbeatCandidate>,
    pub killed_processes: Vec<HeartbeatCandidate>,
    pub process_scan_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeartbeatCandidate {
    pub pid: u32,
    pub ppid: u32,
    pub tty: String,
    pub command: String,
    pub reason: HeartbeatReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeartbeatReason {
    OrphanSleep,
    ShellParentWithSleepChild,
}

pub fn inspect() -> anyhow::Result<CleanupReport> {
    run(CleanupOptions {
        dry_run: true,
        aggressive: false,
    })
}

pub fn run(options: CleanupOptions) -> anyhow::Result<CleanupReport> {
    let state_files = terminal_state::state_files()?;
    let mut legacy_ttys = BTreeSet::new();
    let mut legacy_active_state_files = Vec::new();

    for state in &state_files {
        let Some(target) = &state.target else {
            continue;
        };
        if target.terminal_app.is_none() && target.terminal_id.is_none() {
            legacy_ttys.insert(tty_key(&target.tty));
            legacy_active_state_files.push(state.path.clone());
        }
    }

    let removed_state_files = terminal_state::cleanup_state_files(options.dry_run)?;
    let mut report = CleanupReport {
        state_files: state_files.len(),
        removed_state_files,
        legacy_active_state_files,
        ..CleanupReport::default()
    };

    match scan_heartbeat_candidates(&legacy_ttys, options.aggressive) {
        Ok(candidates) => {
            report.heartbeat_candidates = candidates;
        }
        Err(e) => {
            report.process_scan_error = Some(e.to_string());
            return Ok(report);
        }
    }

    if !options.dry_run {
        for candidate in &report.heartbeat_candidates {
            if kill_process(candidate.pid) {
                report.killed_processes.push(candidate.clone());
            }
        }
    }

    Ok(report)
}

fn scan_heartbeat_candidates(
    legacy_ttys: &BTreeSet<String>,
    aggressive: bool,
) -> anyhow::Result<Vec<HeartbeatCandidate>> {
    if legacy_ttys.is_empty() {
        return Ok(Vec::new());
    }

    let rows = ps_rows()?;
    Ok(candidates_from_rows(&rows, legacy_ttys, aggressive))
}

fn ps_rows() -> anyhow::Result<Vec<PsRow>> {
    let output = Command::new("ps")
        .args(["-axo", "pid,ppid,tty,command"])
        .output()?;
    if !output.status.success() {
        anyhow::bail!("ps failed with status {}", output.status);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_ps_rows(&stdout))
}

fn candidates_from_rows(
    rows: &[PsRow],
    legacy_ttys: &BTreeSet<String>,
    aggressive: bool,
) -> Vec<HeartbeatCandidate> {
    let mut by_pid = HashMap::new();
    for row in rows {
        by_pid.insert(row.pid, row);
    }

    let mut candidates = Vec::new();
    for row in rows {
        if !is_sleep_15(&row.command) || !legacy_ttys.contains(&row.tty) {
            continue;
        }

        if row.ppid == 1 {
            candidates.push(HeartbeatCandidate {
                pid: row.pid,
                ppid: row.ppid,
                tty: row.tty.clone(),
                command: row.command.clone(),
                reason: HeartbeatReason::OrphanSleep,
            });
            continue;
        }

        if aggressive {
            if let Some(parent) = by_pid.get(&row.ppid) {
                if is_shell_command(&parent.command) && parent.tty == row.tty {
                    candidates.push(HeartbeatCandidate {
                        pid: parent.pid,
                        ppid: parent.ppid,
                        tty: parent.tty.clone(),
                        command: parent.command.clone(),
                        reason: HeartbeatReason::ShellParentWithSleepChild,
                    });
                    candidates.push(HeartbeatCandidate {
                        pid: row.pid,
                        ppid: row.ppid,
                        tty: row.tty.clone(),
                        command: row.command.clone(),
                        reason: HeartbeatReason::OrphanSleep,
                    });
                }
            }
        }
    }

    candidates.sort_by_key(|candidate| candidate.pid);
    candidates.dedup_by_key(|candidate| candidate.pid);
    candidates
}

#[cfg(unix)]
fn kill_process(pid: u32) -> bool {
    Command::new("kill")
        .arg(pid.to_string())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn kill_process(_pid: u32) -> bool {
    false
}

fn is_sleep_15(command: &str) -> bool {
    let command = command.trim();
    command == "sleep 15" || command.ends_with("/sleep 15")
}

fn is_shell_command(command: &str) -> bool {
    let command = command.trim();
    command.ends_with("zsh")
        || command.ends_with("/zsh")
        || command.ends_with("bash")
        || command.ends_with("/bash")
        || command.ends_with("fish")
        || command.ends_with("/fish")
}

fn tty_key(tty: &str) -> String {
    tty.trim().trim_start_matches("/dev/").to_string()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PsRow {
    pid: u32,
    ppid: u32,
    tty: String,
    command: String,
}

fn parse_ps_rows(output: &str) -> Vec<PsRow> {
    output.lines().filter_map(parse_ps_row).collect()
}

fn parse_ps_row(line: &str) -> Option<PsRow> {
    let line = line.trim();
    if line.is_empty() || line.starts_with("PID ") {
        return None;
    }
    let mut parts = line.split_whitespace();
    let pid = parts.next()?.parse().ok()?;
    let ppid = parts.next()?.parse().ok()?;
    let tty = parts.next()?.to_string();
    let command = parts.collect::<Vec<_>>().join(" ");
    if command.is_empty() {
        return None;
    }
    Some(PsRow {
        pid,
        ppid,
        tty,
        command,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ps_rows_with_spacey_command() {
        let rows = parse_ps_rows(
            "  PID  PPID TTY      COMMAND\n\
             1234     1 ttys007  sleep 15\n\
             5678  1234 ttys007  /bin/zsh -l\n",
        );
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].pid, 1234);
        assert_eq!(rows[0].command, "sleep 15");
        assert_eq!(rows[1].command, "/bin/zsh -l");
    }

    #[test]
    fn finds_only_orphan_sleep_by_default() {
        let rows = parse_ps_rows(
            "  PID  PPID TTY      COMMAND\n\
             1000     1 ttys007  sleep 15\n\
             1001  2000 ttys008  sleep 15\n\
             1002  2000 ttys007  sleep 20\n",
        );
        let mut ttys = BTreeSet::new();
        ttys.insert("ttys007".to_string());

        let candidates = candidates_from_rows(&rows, &ttys, false);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].pid, 1000);
        assert_eq!(candidates[0].reason, HeartbeatReason::OrphanSleep);
    }

    #[test]
    fn aggressive_mode_includes_shell_parent() {
        let rows = parse_ps_rows(
            "  PID  PPID TTY      COMMAND\n\
             2000   500 ttys007  -/bin/zsh\n\
             2001  2000 ttys007  sleep 15\n",
        );
        let mut ttys = BTreeSet::new();
        ttys.insert("ttys007".to_string());

        let candidates = candidates_from_rows(&rows, &ttys, true);
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].pid, 2000);
        assert_eq!(candidates[1].pid, 2001);
    }
}
