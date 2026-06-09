pub mod ai_tui;
pub mod clipboard;
pub mod config;
pub mod detector;
pub mod lang;
pub mod notify;
pub mod proc;
pub mod protected_paste;
pub mod redact_cli;
pub mod shell_rc;
pub mod stats;
pub mod targets;
pub mod updater;

/// Product name. Single source so window/notification/shortcut labels stay
/// consistent; deliberately not localized (it is a proper noun).
pub const APP_NAME: &str = "BeforePaste";

pub use detector::deep_scan::DeepFinding;
pub use detector::patterns::{bucket_patterns, SecretPattern, Severity};
pub use detector::{DetectionResult, Detector};
