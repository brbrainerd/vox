//! Runtime configuration carried at construction.
//!
//! A `VoxConfig` is produced once by the host (Tauri desktop shell on
//! desktop, the Expo Module wrapping the uniffi-bridged runtime on mobile)
//! and passed into every runtime subsystem. The presence of this typed
//! value — instead of process-wide globals — is what makes it possible to
//! run multiple Vox runtimes in one process (e.g. for tests).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::profile::RuntimeProfile;

/// Top-level configuration for a Vox runtime instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoxConfig {
    /// Where the workflow journal, durable state, and per-app SQLite files live.
    ///
    /// - **Desktop:** typically `~/.vox/<app-id>/data/` (Tauri-managed).
    /// - **Mobile:** the platform's app-private documents directory
    ///   (`NSDocumentDirectory` on iOS, `getFilesDir()` on Android).
    pub data_dir: PathBuf,
    /// Where on-device ML model files live.
    ///
    /// - **Desktop:** `~/.vox/<app-id>/models/`.
    /// - **Mobile:** subdirectory of `data_dir` populated by `expo-asset`
    ///   on first launch.
    pub model_dir: PathBuf,
    /// Log level for the `tracing` subscriber the runtime installs.
    ///
    /// Stored as a string for serde simplicity; the runtime translates to
    /// [`tracing::Level`] at startup. Acceptable values are the standard
    /// `error` / `warn` / `info` / `debug` / `trace`.
    pub log_level: String,
    /// Profile dispatching every per-target policy (scheduler, journal, ML).
    pub profile: RuntimeProfile,
}

impl VoxConfig {
    /// Build a desktop-profile config with sensible defaults.
    ///
    /// Both directories live under `~/.vox/default/` unless overridden via
    /// the returned config's mutable fields.
    pub fn desktop() -> Self {
        let root = home_dir().join(".vox").join("default");
        Self {
            data_dir: root.join("data"),
            model_dir: root.join("models"),
            log_level: "info".to_string(),
            profile: RuntimeProfile::Desktop,
        }
    }

    /// Build a mobile-profile config rooted under `data_dir`.
    ///
    /// `data_dir` is the platform's app-private documents directory; the
    /// runtime creates `data_dir/data/` and `data_dir/models/` subdirs as
    /// needed at startup. Log level defaults to `info`.
    pub fn mobile(data_dir: PathBuf) -> Self {
        let model_dir = data_dir.join("models");
        let data_subdir = data_dir.join("data");
        Self {
            data_dir: data_subdir,
            model_dir,
            log_level: "info".to_string(),
            profile: RuntimeProfile::Mobile,
        }
    }

    /// Parse [`Self::log_level`] into a typed [`tracing::Level`].
    ///
    /// Returns `tracing::Level::INFO` when the stored string is unrecognized
    /// (logging a runtime warning is the caller's responsibility).
    pub fn log_level_parsed(&self) -> tracing::Level {
        match self.log_level.to_ascii_lowercase().as_str() {
            "error" => tracing::Level::ERROR,
            "warn" => tracing::Level::WARN,
            "info" => tracing::Level::INFO,
            "debug" => tracing::Level::DEBUG,
            "trace" => tracing::Level::TRACE,
            _ => tracing::Level::INFO,
        }
    }
}

/// Resolve the current user's home directory.
///
/// Looks at `$HOME` (Unix-style hosts, including Git Bash on Windows),
/// `$USERPROFILE` (cmd / native PowerShell), or falls back to the current
/// working directory if neither is set. Using the CWD avoids `unwrap()`
/// panics in unusual hosts (CI, headless containers).
fn home_dir() -> PathBuf {
    if let Ok(h) = std::env::var("HOME")
        && !h.is_empty() {
            return PathBuf::from(h);
        }
    if let Ok(p) = std::env::var("USERPROFILE")
        && !p.is_empty() {
            return PathBuf::from(p);
        }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_defaults_to_desktop_profile() {
        let cfg = VoxConfig::desktop();
        assert_eq!(cfg.profile, RuntimeProfile::Desktop);
        assert_eq!(cfg.log_level, "info");
        assert!(cfg.data_dir.ends_with("data"));
        assert!(cfg.model_dir.ends_with("models"));
    }

    #[test]
    fn mobile_uses_provided_data_root() {
        let cfg = VoxConfig::mobile(PathBuf::from("/var/mobile/app/Documents"));
        assert_eq!(cfg.profile, RuntimeProfile::Mobile);
        assert_eq!(
            cfg.data_dir,
            PathBuf::from("/var/mobile/app/Documents/data")
        );
        assert_eq!(
            cfg.model_dir,
            PathBuf::from("/var/mobile/app/Documents/models")
        );
    }

    #[test]
    fn log_level_parses_case_insensitively() {
        let mut cfg = VoxConfig::desktop();
        for (input, expected) in &[
            ("error", tracing::Level::ERROR),
            ("WARN", tracing::Level::WARN),
            ("Info", tracing::Level::INFO),
            ("debug", tracing::Level::DEBUG),
            ("TRACE", tracing::Level::TRACE),
        ] {
            cfg.log_level = (*input).to_string();
            assert_eq!(cfg.log_level_parsed(), *expected, "input: {input}");
        }
    }

    #[test]
    fn log_level_falls_back_to_info_for_garbage() {
        let mut cfg = VoxConfig::desktop();
        cfg.log_level = "verbose-please".to_string();
        assert_eq!(cfg.log_level_parsed(), tracing::Level::INFO);
    }

    #[test]
    fn config_round_trips_through_json() {
        let cfg = VoxConfig::mobile(PathBuf::from("/tmp/vox-mobile-test"));
        let json = serde_json::to_string(&cfg).unwrap();
        let decoded: VoxConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, cfg);
    }
}
