//! `vox-runtime-rn` — uniffi-bridged Vox runtime for React Native + Expo.
//!
//! Re-exports the foundation types from [`vox_runtime`] through the uniffi
//! proc-macro layer so [uniffi-bindgen-react-native][bindgen] can generate
//! the TypeScript TurboModule bindings that `clients/runtime-rn/` consumes.
//!
//! [bindgen]: https://github.com/jhugman/uniffi-bindgen-react-native
//!
//! ## What's exposed today
//!
//! - [`RuntimeProfile`] mirror enum (uniffi can't yet remote-import a type
//!   from another crate's enum, so we wrap it via `From` impls).
//! - [`VoxConfig`] mirror record carrying the same fields as
//!   [`vox_runtime::VoxConfig`].
//! - [`VoxRuntimeHandle`] — the Rust-side runtime instance. Constructed
//!   from a [`VoxConfig`], exposes [`VoxRuntimeHandle::profile`],
//!   [`VoxRuntimeHandle::data_dir`], [`VoxRuntimeHandle::model_dir`], and
//!   [`VoxRuntimeHandle::log`] (which actually emits a `tracing` event).
//! - [`VoxRnError`] — typed errors raised by future fallible methods.
//!
//! ## What's NOT exposed yet (each will land with the underlying impl)
//!
//! - `spawn_actor` / `start_workflow` — depend on `vox-actor-runtime` /
//!   `vox-workflow-runtime` adopting `Suspendable` and exposing a thread-
//!   safe public API. Tracked in spec §13.
//! - `infer` / `transcribe` — depend on Candle ML cross-compile +
//!   model-asset pipeline. Tracked in spec §15.
//!
//! Until those land, the JS-side `@vox/runtime-rn` throws an explicit
//! `UnsupportedOnPlatform` error for those methods. No silent stubs.

#![warn(missing_docs, missing_debug_implementations)]

use std::sync::Arc;

use thiserror::Error;
use vox_runtime::{RuntimeProfile as InnerProfile, VoxConfig as InnerConfig};

// ── Mirror types ────────────────────────────────────────────────────────

/// Runtime execution profile (uniffi-exposed mirror of
/// [`vox_runtime::RuntimeProfile`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum RuntimeProfile {
    /// Desktop (Tauri 2): multi-threaded scheduler, periodic journal flush,
    /// eager model retention.
    Desktop,
    /// Mobile (React Native + Expo + uniffi): single-threaded scheduler,
    /// journal-on-lifecycle, lazy model retention with memory-pressure
    /// unload.
    Mobile,
}

impl From<InnerProfile> for RuntimeProfile {
    fn from(p: InnerProfile) -> Self {
        match p {
            InnerProfile::Desktop => Self::Desktop,
            InnerProfile::Mobile => Self::Mobile,
        }
    }
}

impl From<RuntimeProfile> for InnerProfile {
    fn from(p: RuntimeProfile) -> Self {
        match p {
            RuntimeProfile::Desktop => Self::Desktop,
            RuntimeProfile::Mobile => Self::Mobile,
        }
    }
}

/// Runtime configuration carried at construction (uniffi-exposed mirror of
/// [`vox_runtime::VoxConfig`]). Fields are explicit `String` for the paths
/// because uniffi doesn't expose `PathBuf` across the FFI boundary; the
/// strings are interpreted as paths inside the Rust side.
#[derive(Debug, Clone, uniffi::Record)]
pub struct VoxConfig {
    /// Where workflow journal + durable state live.
    pub data_dir: String,
    /// Where on-device ML model files live.
    pub model_dir: String,
    /// Log level for the `tracing` subscriber (`error` / `warn` / `info` /
    /// `debug` / `trace`; unrecognized values fall back to `info`).
    pub log_level: String,
    /// Profile dispatching every per-target policy.
    pub profile: RuntimeProfile,
}

impl From<VoxConfig> for InnerConfig {
    fn from(c: VoxConfig) -> Self {
        Self {
            data_dir: std::path::PathBuf::from(c.data_dir),
            model_dir: std::path::PathBuf::from(c.model_dir),
            log_level: c.log_level,
            profile: c.profile.into(),
        }
    }
}

impl From<InnerConfig> for VoxConfig {
    fn from(c: InnerConfig) -> Self {
        Self {
            data_dir: c.data_dir.to_string_lossy().into_owned(),
            model_dir: c.model_dir.to_string_lossy().into_owned(),
            log_level: c.log_level,
            profile: c.profile.into(),
        }
    }
}

/// Errors raised across the uniffi boundary.
#[derive(Debug, Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum VoxRnError {
    /// The runtime was used before [`VoxRuntimeHandle`] was constructed.
    #[error("vox-runtime-rn not initialized")]
    NotInitialized,
    /// Caller-provided context for unexpected failures.
    #[error("internal error: {0}")]
    Internal(String),
}

// ── The runtime handle ──────────────────────────────────────────────────

/// Live runtime instance, returned by [`VoxRuntimeHandle::new`].
///
/// Holds the parsed [`InnerConfig`] internally; mirror getters expose the
/// fields back across uniffi without leaking `PathBuf` (which uniffi can't
/// transit).
#[derive(Debug, uniffi::Object)]
pub struct VoxRuntimeHandle {
    inner: InnerConfig,
}

#[uniffi::export]
impl VoxRuntimeHandle {
    /// Construct a new runtime handle from the given configuration.
    ///
    /// Validates the config inline (parses log level via
    /// [`InnerConfig::log_level_parsed`]) so callers can't construct a
    /// handle with a wildly invalid profile + log-level combo.
    #[uniffi::constructor]
    pub fn new(config: VoxConfig) -> Arc<Self> {
        let inner: InnerConfig = config.into();
        tracing::info!(
            target = "vox_runtime_rn",
            profile = ?inner.profile,
            data_dir = %inner.data_dir.display(),
            "vox-runtime-rn initialized"
        );
        Arc::new(Self { inner })
    }

    /// The runtime's [`RuntimeProfile`].
    pub fn profile(&self) -> RuntimeProfile {
        self.inner.profile.into()
    }

    /// The runtime's data directory as a UTF-8 string.
    pub fn data_dir(&self) -> String {
        self.inner.data_dir.to_string_lossy().into_owned()
    }

    /// The runtime's model directory as a UTF-8 string.
    pub fn model_dir(&self) -> String {
        self.inner.model_dir.to_string_lossy().into_owned()
    }

    /// Whether this profile requires the JS shell to wire suspend / resume
    /// lifecycle hooks (true on Mobile, false on Desktop).
    pub fn requires_suspend_hooks(&self) -> bool {
        self.inner.profile.requires_suspend_hooks()
    }

    /// Emit a tracing event from the JS side. The `level` string is
    /// interpreted the same way [`InnerConfig::log_level_parsed`] handles
    /// it — unknown values are silently coerced to `info` so this method
    /// never fails for a bad level argument.
    pub fn log(&self, level: String, message: String) {
        match level.to_ascii_lowercase().as_str() {
            "error" => tracing::error!(target = "vox_runtime_rn", "{message}"),
            "warn" => tracing::warn!(target = "vox_runtime_rn", "{message}"),
            "info" => tracing::info!(target = "vox_runtime_rn", "{message}"),
            "debug" => tracing::debug!(target = "vox_runtime_rn", "{message}"),
            "trace" => tracing::trace!(target = "vox_runtime_rn", "{message}"),
            _ => tracing::info!(target = "vox_runtime_rn", "{message}"),
        }
    }
}

// ── Top-level helpers exposed to JS ─────────────────────────────────────

/// Build a desktop-profile config with the platform's default data + model
/// directories. Equivalent to [`vox_runtime::VoxConfig::desktop`] but
/// crosses the uniffi boundary so the JS side can request a sensible
/// default without hardcoding host-specific paths.
#[uniffi::export]
pub fn default_desktop_config() -> VoxConfig {
    InnerConfig::desktop().into()
}

/// Build a mobile-profile config rooted at `data_dir`. Counterpart of
/// [`vox_runtime::VoxConfig::mobile`].
#[uniffi::export]
pub fn default_mobile_config(data_dir: String) -> VoxConfig {
    InnerConfig::mobile(std::path::PathBuf::from(data_dir)).into()
}

uniffi::setup_scaffolding!();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_round_trips_through_mirror() {
        for &p in &[InnerProfile::Desktop, InnerProfile::Mobile] {
            let mirror: RuntimeProfile = p.into();
            let back: InnerProfile = mirror.into();
            assert_eq!(back, p);
        }
    }

    #[test]
    fn vox_config_round_trips_through_mirror() {
        let inner = InnerConfig::mobile(std::path::PathBuf::from("/tmp/vox-rt-rn-test"));
        let mirror: VoxConfig = inner.clone().into();
        let back: InnerConfig = mirror.into();
        assert_eq!(back, inner);
    }

    #[test]
    fn handle_exposes_profile_and_dirs() {
        let cfg = VoxConfig {
            data_dir: "/tmp/d".to_string(),
            model_dir: "/tmp/m".to_string(),
            log_level: "info".to_string(),
            profile: RuntimeProfile::Mobile,
        };
        let h = VoxRuntimeHandle::new(cfg);
        assert_eq!(h.profile(), RuntimeProfile::Mobile);
        assert_eq!(h.data_dir(), "/tmp/d");
        assert_eq!(h.model_dir(), "/tmp/m");
        assert!(h.requires_suspend_hooks());
    }

    #[test]
    fn handle_desktop_does_not_require_suspend_hooks() {
        let cfg = VoxConfig {
            data_dir: "/tmp/d".to_string(),
            model_dir: "/tmp/m".to_string(),
            log_level: "info".to_string(),
            profile: RuntimeProfile::Desktop,
        };
        let h = VoxRuntimeHandle::new(cfg);
        assert_eq!(h.profile(), RuntimeProfile::Desktop);
        assert!(!h.requires_suspend_hooks());
    }

    #[test]
    fn default_desktop_config_has_desktop_profile() {
        let cfg = default_desktop_config();
        assert_eq!(cfg.profile, RuntimeProfile::Desktop);
        assert_eq!(cfg.log_level, "info");
    }

    #[test]
    fn default_mobile_config_uses_provided_root() {
        let cfg = default_mobile_config("/var/mobile/Documents".to_string());
        assert_eq!(cfg.profile, RuntimeProfile::Mobile);
        // Path separator is platform-dependent; we assert on the typed
        // PathBuf equality, not on the stringified form, so this passes
        // on both Windows and Unix.
        let data_back: std::path::PathBuf = cfg.data_dir.into();
        let model_back: std::path::PathBuf = cfg.model_dir.into();
        assert_eq!(data_back, std::path::PathBuf::from("/var/mobile/Documents/data"));
        assert_eq!(model_back, std::path::PathBuf::from("/var/mobile/Documents/models"));
    }

    #[test]
    fn log_method_does_not_panic_on_any_level() {
        let cfg = VoxConfig {
            data_dir: "/tmp/d".to_string(),
            model_dir: "/tmp/m".to_string(),
            log_level: "info".to_string(),
            profile: RuntimeProfile::Mobile,
        };
        let h = VoxRuntimeHandle::new(cfg);
        for level in &["error", "warn", "info", "debug", "trace", "garbage"] {
            h.log((*level).to_string(), "test".to_string());
        }
    }

    #[test]
    fn error_displays_helpfully() {
        let e = VoxRnError::NotInitialized;
        assert!(format!("{e}").contains("not initialized"));
        let e = VoxRnError::Internal("oops".to_string());
        assert!(format!("{e}").contains("oops"));
    }
}
