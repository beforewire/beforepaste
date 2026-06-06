use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::config;

const CACHE_FILE: &str = "target-state.json";
const DEFAULT_TTL_SECS: u64 = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetSnapshot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub updated_at: u64,
    pub expires_at: u64,
}

pub fn write(reason: Option<&str>) -> anyhow::Result<()> {
    write_with_ttl(reason, DEFAULT_TTL_SECS)
}

fn write_with_ttl(reason: Option<&str>, ttl_secs: u64) -> anyhow::Result<()> {
    let now = now_secs();
    let snapshot = TargetSnapshot {
        reason: reason.map(str::to_string),
        updated_at: now,
        expires_at: now.saturating_add(ttl_secs.max(1)),
    };
    let bytes = serde_json::to_vec_pretty(&snapshot)?;
    config::atomic_write(&cache_path(), &bytes)?;
    Ok(())
}

#[cfg(test)]
fn read_active() -> anyhow::Result<Option<String>> {
    let data = match std::fs::read_to_string(cache_path()) {
        Ok(data) => data,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let snapshot: TargetSnapshot = serde_json::from_str(&data)?;
    if snapshot.expires_at <= now_secs() {
        return Ok(None);
    }
    Ok(snapshot.reason)
}

fn cache_path() -> std::path::PathBuf {
    config::base_dir().join(CACHE_FILE)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::path::Path;

    struct ConfigHomeGuard {
        saved: Option<std::ffi::OsString>,
    }

    impl ConfigHomeGuard {
        fn set(path: &Path) -> Self {
            let saved = std::env::var_os("BEFOREPASTE_CONFIG_HOME");
            std::env::set_var("BEFOREPASTE_CONFIG_HOME", path);
            Self { saved }
        }
    }

    impl Drop for ConfigHomeGuard {
        fn drop(&mut self) {
            if let Some(saved) = self.saved.take() {
                std::env::set_var("BEFOREPASTE_CONFIG_HOME", saved);
            } else {
                std::env::remove_var("BEFOREPASTE_CONFIG_HOME");
            }
        }
    }

    #[test]
    #[serial]
    fn active_cache_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = ConfigHomeGuard::set(dir.path());
        write_with_ttl(Some("cli:codex"), 60).unwrap();
        assert_eq!(read_active().unwrap(), Some("cli:codex".to_string()));
    }

    #[test]
    #[serial]
    fn missing_or_stale_cache_is_not_target() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = ConfigHomeGuard::set(dir.path());
        assert_eq!(read_active().unwrap(), None);
        write_with_ttl(Some("cli:codex"), 1).unwrap();
        let mut snapshot: TargetSnapshot =
            serde_json::from_str(&std::fs::read_to_string(cache_path()).unwrap()).unwrap();
        snapshot.expires_at = 0;
        config::atomic_write(
            &cache_path(),
            &serde_json::to_vec_pretty(&snapshot).unwrap(),
        )
        .unwrap();
        assert_eq!(read_active().unwrap(), None);
    }

    #[test]
    #[serial]
    fn recent_non_target_is_not_target() {
        let dir = tempfile::tempdir().unwrap();
        let _guard = ConfigHomeGuard::set(dir.path());
        write_with_ttl(None, 60).unwrap();
        assert_eq!(read_active().unwrap(), None);
    }
}
