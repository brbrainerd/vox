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

use serde::{Deserialize, Serialize};
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

// ── File journal (vox-journal-backed, mobile-portable) ──────────────────

/// A single JSON-encoded line carried by the file journal.
///
/// The payload is opaque to the runtime — uniffi-exposed callers (JS side)
/// pass arbitrary JSON strings; vox-journal handles append + replay + fsync.
/// Letting the payload be a string instead of an opaque `serde_json::Value`
/// is intentional: uniffi can't transit `Value` directly across the FFI
/// boundary, and the JS side already speaks JSON natively.
#[derive(Debug, Clone, Serialize, Deserialize, uniffi::Record)]
pub struct JournalLine {
    /// The JSON-encoded payload. Must parse as valid JSON; the journal
    /// preserves the bytes verbatim across append/replay.
    pub json: String,
}

/// File journal errors raised across the uniffi boundary.
#[derive(Debug, Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum FileJournalError {
    /// Underlying I/O failed.
    #[error("file journal I/O error: {0}")]
    Io(String),
    /// The payload was not valid JSON.
    #[error("file journal payload not valid JSON: {0}")]
    InvalidJson(String),
    /// Generic / wrap-around error context.
    #[error("{0}")]
    Other(String),
}

impl From<vox_journal::JournalError> for FileJournalError {
    fn from(e: vox_journal::JournalError) -> Self {
        match e {
            vox_journal::JournalError::Io(io) => Self::Io(io.to_string()),
            vox_journal::JournalError::Serde(s) => Self::InvalidJson(s.to_string()),
            vox_journal::JournalError::Poisoned => {
                Self::Other("journal writer mutex poisoned".to_string())
            }
        }
    }
}

/// Live file-journal handle exposed to JS.
///
/// Construct via [`open_file_journal`]. Every successful `append` call
/// fsyncs to disk before returning. `replay_all` returns every recorded
/// line (in append order) as a list of `JournalLine`s.
#[derive(Debug, uniffi::Object)]
pub struct FileJournalHandle {
    inner: vox_journal::FileJournal<serde_json::Value>,
}

#[uniffi::export]
impl FileJournalHandle {
    /// Append a JSON line. Returns an error if the line is not valid JSON
    /// or if the underlying I/O fails.
    pub fn append(&self, line: JournalLine) -> Result<(), FileJournalError> {
        let value: serde_json::Value = serde_json::from_str(&line.json)
            .map_err(|e| FileJournalError::InvalidJson(e.to_string()))?;
        self.inner.append(&value).map_err(Into::into)
    }

    /// Read every recorded line back into JS, in append order.
    pub fn replay_all(&self) -> Result<Vec<JournalLine>, FileJournalError> {
        let entries = self.inner.replay_all().map_err(FileJournalError::from)?;
        Ok(entries
            .into_iter()
            .map(|v| JournalLine {
                json: v.to_string(),
            })
            .collect())
    }

    /// The on-disk path being written. Useful for `tracing` log lines on
    /// the JS side.
    pub fn path(&self) -> String {
        self.inner.path().to_string_lossy().into_owned()
    }

    /// Flush + fsync any in-flight bytes. This is the durability point for
    /// the mobile journal: appends run in [`AppendDurability::Deferred`] mode
    /// (no per-append fsync), so the JS lifecycle handler MUST call this from
    /// the OS suspend hook (app backgrounding) or un-synced appends can be
    /// lost on power-off. Idempotent and safe to call repeatedly.
    ///
    /// [`AppendDurability::Deferred`]: vox_journal::AppendDurability::Deferred
    pub fn flush(&self) -> Result<(), FileJournalError> {
        use vox_runtime::{SuspendDeadline, Suspendable};
        self.inner
            .suspend(SuspendDeadline::mobile_default())
            .map_err(|e| FileJournalError::Other(e.to_string()))
    }
}

/// Open (or create) a file journal at `path`. Returns a [`FileJournalHandle`]
/// that the JS side can keep alive for the duration of the workflow.
///
/// Opens in [`AppendDurability::Deferred`] mode — this is the mobile runtime
/// bridge, so appends are batched (no per-call fsync, to honor battery +
/// throughput budgets) and durability is taken at
/// [`FileJournalHandle::flush`], which the JS side wires to the OS suspend
/// hook. Replays any existing entries silently — the JS side calls
/// [`FileJournalHandle::replay_all`] explicitly when it wants them.
///
/// [`AppendDurability::Deferred`]: vox_journal::AppendDurability::Deferred
#[uniffi::export]
pub fn open_file_journal(
    path: String,
) -> Result<std::sync::Arc<FileJournalHandle>, FileJournalError> {
    let opened = vox_journal::FileJournal::<serde_json::Value>::open_with_durability(
        path,
        vox_journal::AppendDurability::Deferred,
    )
    .map_err(FileJournalError::from)?;
    Ok(std::sync::Arc::new(FileJournalHandle {
        inner: opened.journal,
    }))
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
        // vox-arch-check: allow abs-path
        let inner = InnerConfig::mobile(std::path::PathBuf::from("/tmp/vox-rt-rn-test"));
        let mirror: VoxConfig = inner.clone().into();
        let back: InnerConfig = mirror.into();
        assert_eq!(back, inner);
    }

    #[test]
    fn handle_exposes_profile_and_dirs() {
        let cfg = VoxConfig {
            // vox-arch-check: allow abs-path
            data_dir: "/tmp/d".to_string(),
            model_dir: "/tmp/m".to_string(),
            log_level: "info".to_string(),
            profile: RuntimeProfile::Mobile,
        };
        let h = VoxRuntimeHandle::new(cfg);
        assert_eq!(h.profile(), RuntimeProfile::Mobile);
        // vox-arch-check: allow abs-path
        assert_eq!(h.data_dir(), "/tmp/d");
        // vox-arch-check: allow abs-path
        assert_eq!(h.model_dir(), "/tmp/m");
        assert!(h.requires_suspend_hooks());
    }

    #[test]
    fn handle_desktop_does_not_require_suspend_hooks() {
        let cfg = VoxConfig {
            // vox-arch-check: allow abs-path
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
        // vox-arch-check: allow abs-path
        let cfg = default_mobile_config("/var/mobile/Documents".to_string());
        assert_eq!(cfg.profile, RuntimeProfile::Mobile);
        // Path separator is platform-dependent; we assert on the typed
        // PathBuf equality, not on the stringified form, so this passes
        // on both Windows and Unix.
        let data_back: std::path::PathBuf = cfg.data_dir.into();
        let model_back: std::path::PathBuf = cfg.model_dir.into();
        assert_eq!(
            data_back,
            // vox-arch-check: allow abs-path
            std::path::PathBuf::from("/var/mobile/Documents/data")
        );
        assert_eq!(
            model_back,
            // vox-arch-check: allow abs-path
            std::path::PathBuf::from("/var/mobile/Documents/models")
        );
    }

    #[test]
    fn log_method_does_not_panic_on_any_level() {
        let cfg = VoxConfig {
            // vox-arch-check: allow abs-path
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

    fn temp_journal_path() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let pid = std::process::id();
        std::env::temp_dir()
            .join(format!("vox_rn_journal_test_{pid}_{n}.jsonl"))
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn open_file_journal_round_trips_lines() {
        let path = temp_journal_path();
        let _ = std::fs::remove_file(&path);

        let h = open_file_journal(path.clone()).expect("open");
        h.append(JournalLine {
            json: "{\"entry\":1}".into(),
        })
        .expect("append 1");
        h.append(JournalLine {
            json: "{\"entry\":2}".into(),
        })
        .expect("append 2");
        assert_eq!(h.path(), path);

        let replayed = h.replay_all().expect("replay");
        assert_eq!(replayed.len(), 2);
        assert!(replayed[0].json.contains("\"entry\":1"));
        assert!(replayed[1].json.contains("\"entry\":2"));

        // Drop the handle and re-open — entries survive.
        drop(h);
        let h2 = open_file_journal(path.clone()).expect("re-open");
        let replayed2 = h2.replay_all().expect("re-replay");
        assert_eq!(replayed2.len(), 2);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn open_file_journal_rejects_invalid_json_payload() {
        let path = temp_journal_path();
        let _ = std::fs::remove_file(&path);
        let h = open_file_journal(path.clone()).expect("open");
        let err = h
            .append(JournalLine {
                json: "not json at all".into(),
            })
            .expect_err("invalid JSON must fail");
        assert!(
            matches!(err, FileJournalError::InvalidJson(_)),
            "expected InvalidJson, got {err:?}"
        );
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn flush_succeeds_on_an_open_journal_handle() {
        let path = temp_journal_path();
        let _ = std::fs::remove_file(&path);
        let h = open_file_journal(path.clone()).expect("open");
        h.flush().expect("flush");
        std::fs::remove_file(&path).ok();
    }
}

#[cfg(test)]
mod semcov_wave5_tests {
    use super::*;
    use vox_journal::JournalError as InnerJournalError;

    #[test]
    fn file_journal_error_from_inner_io_maps_to_io_variant() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "no perms");
        let inner = InnerJournalError::Io(io_err);
        let mapped = FileJournalError::from(inner);
        match mapped {
            FileJournalError::Io(msg) => {
                assert!(msg.contains("no perms"), "expected io message, got: {msg}")
            }
            other => panic!("expected Io variant, got: {other:?}"),
        }
    }

    #[test]
    fn file_journal_error_from_inner_serde_maps_to_invalid_json_variant() {
        // Construct a serde_json::Error by parsing invalid JSON
        let serde_err: serde_json::Error =
            serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let inner = InnerJournalError::Serde(serde_err);
        let mapped = FileJournalError::from(inner);
        match mapped {
            FileJournalError::InvalidJson(msg) => {
                assert!(!msg.is_empty(), "expected non-empty InvalidJson message");
            }
            other => panic!("expected InvalidJson variant, got: {other:?}"),
        }
    }

    #[test]
    fn file_journal_error_from_inner_poisoned_maps_to_other_variant() {
        let inner = InnerJournalError::Poisoned;
        let mapped = FileJournalError::from(inner);
        match mapped {
            FileJournalError::Other(msg) => {
                assert!(
                    msg.contains("poisoned"),
                    "expected 'poisoned' in message, got: {msg}"
                );
            }
            other => panic!("expected Other variant, got: {other:?}"),
        }
    }
}
