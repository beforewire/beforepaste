//! `watch` — background auto-redact loop for AI targets..
//!
//! While an AI app/site is frontmost (see `target`), the clipboard never holds
//! raw secrets: on a clipboard change we scrub via the upstream engine
//! (`redact_with`) and write the redacted copy back; when focus leaves the AI
//! target we restore the original (race-guarded: only if the clipboard still
//! holds exactly the redacted copy we wrote, so a newer copy is never clobbered).
//!
//! Fully additive: reuses `crate::clipboard::ClipboardMonitor` and the library
//! `redact_with`/`Detector`/`Config`. No upstream core files modified.

mod target;
pub(crate) mod target_cache;
pub(crate) mod terminal_cleanup;
pub(crate) mod terminal_state;

use std::thread::sleep;
use std::time::{Duration, Instant};

use crate::clipboard::ClipboardMonitor;
use beforepaste::config::Config;
use beforepaste::detector::Detector;
use beforepaste::redact_cli::redact_with;

pub fn run(target_cache_only: bool) -> anyhow::Result<()> {
    let cfg = Config::load();
    let detector = Detector::from_config(&cfg);
    let mut clip = if target_cache_only {
        None
    } else {
        Some(ClipboardMonitor::new(0).map_err(|e| anyhow::anyhow!("clipboard init failed: {e}"))?)
    };

    if target_cache_only {
        eprintln!(
            "beforepaste watch --target-cache-only — publish current AI target for protected paste. Ctrl-C to stop."
        );
    } else {
        eprintln!(
            "beforepaste watch — auto-redact secrets while an AI app/site is frontmost \
             (restored when you leave). Ctrl-C to stop."
        );
    }

    let interval = Duration::from_millis(400);
    let mut holding: Option<String> = None; // original (raw) text we swapped away
    let mut our_write: Option<String> = None; // the redacted text we wrote
    let mut last_target_log: Option<String> = None; // for observable target-change logging
    let mut last_cache_target: Option<String> = None;
    let mut last_cache_write = Instant::now() - Duration::from_secs(60);
    let mut missed_target_ticks = 0_u8; // tolerate brief terminal-title flicker

    loop {
        sleep(interval);
        let detected = target::current();
        let target = match detected {
            target::Detection::Ai(reason) => Some(reason),
            target::Detection::Terminal | target::Detection::Other => None,
        };

        if target != last_cache_target || last_cache_write.elapsed() >= Duration::from_secs(1) {
            let _ = target_cache::write(target.as_deref());
            last_cache_target = target.clone();
            last_cache_write = Instant::now();
        }

        // Observable target logging: switch focus between apps and watch this.
        if target.as_deref() != last_target_log.as_deref() {
            match target.as_deref() {
                Some(r) => eprintln!("[beforepaste] target → {r}"),
                None => eprintln!("[beforepaste] target → (not an AI target)"),
            }
            last_target_log = target.clone();
        }

        if target_cache_only {
            continue;
        }

        let Some(clip) = clip.as_mut() else {
            continue;
        };
        let text = clip.read_text();

        match target {
            Some(reason) => {
                missed_target_ticks = 0;
                let Some(t) = text else { continue };
                // No last-seen gate: content copied BEFORE entering the AI target must still be
                // scrubbed on entry (the main use case: copy elsewhere, switch to AI, paste).
                // redact_with is idempotent, so once swapped the redacted text is a no-op next tick.
                let (redacted, names) = redact_with(&detector, &cfg, &t);
                if redacted != t {
                    holding = Some(t.clone());
                    our_write = Some(redacted.clone());
                    if clip.replace_text(&redacted).is_ok() {
                        eprintln!(
                            "[beforepaste] 🛡 redacted before {reason}: {}",
                            names.join(", ")
                        );
                    }
                } else if our_write.as_deref() != Some(t.as_str()) {
                    // clean text that isn't our own redacted write → forget any pending swap
                    holding = None;
                    our_write = None;
                }
            }
            None => {
                missed_target_ticks = missed_target_ticks.saturating_add(1);
                if missed_target_ticks < 3 {
                    continue;
                }
                // Left the AI target. Restore the original ONLY if the clipboard
                // still holds exactly the redacted copy we wrote (else a newer
                // copy happened — never clobber it).
                if holding.is_some() {
                    if text.as_deref() == our_write.as_deref() {
                        if let Some(orig) = holding.take() {
                            let _ = clip.replace_text(&orig);
                        }
                    } else {
                        holding = None;
                    }
                    our_write = None;
                }
            }
        }
    }
}
