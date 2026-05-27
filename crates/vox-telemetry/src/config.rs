//! `TelemetryConfig`: read once at startup, governs which sinks and categories are active.
//!
//! Resolution order (highest wins):
//!   1. `/etc/vox/telemetry-policy.toml` — org-level hard-off (Phase D)
//!   2. `~/.config/vox/config.toml`        — user preference (Phase D)
//!   3. `VOX_TELEMETRY`                    — master on/off/debug (Phase D)
//!   4. Legacy per-category env vars        — compat shim
//!   5. Default                             — local collection on, remote upload off
//!
//! Phases B–D complete the config loading. In Phase A, only `from_env_legacy` is
//! implemented to keep the existing per-category env-var gates working.

/// Master telemetry configuration.
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    /// Master switch. When false, `record_event!` is a guaranteed no-op.
    pub enabled: bool,
    /// Whether to attempt remote upload via the spool. Requires Clavis credentials (ADR 023).
    pub remote_upload: bool,
    /// Gather research_metrics (existing categories: benchmark, syntax_k, socrates, etc.).
    pub research_metrics: bool,
    /// Gather per-LLM-call model performance events (Phase B).
    pub model_calls: bool,
    /// Gather agent dispatch + task root summary (Phase C).
    pub agent_orchestration: bool,
    /// Gather build summary events (Phase D).
    pub build: bool,
    /// Gather subsystem error events (Phase D).
    pub errors: bool,
    /// When true (`VOX_TELEMETRY=debug`), emit every event as JSON to stderr for
    /// developer inspection. The CLI registers a `StdoutSink` (writing to stderr)
    /// when this flag is set. Default: false.
    pub debug_to_stderr: bool,
}

impl TelemetryConfig {
    /// Returns the default config: local collection on, all categories on, remote upload off.
    pub fn default_on() -> Self {
        Self {
            enabled: true,
            remote_upload: false,
            research_metrics: true,
            model_calls: true,
            agent_orchestration: true,
            build: true,
            errors: true,
            debug_to_stderr: false,
        }
    }

    /// Returns an all-off config (used for tests and when master switch is disabled).
    pub fn all_off() -> Self {
        Self {
            enabled: false,
            remote_upload: false,
            research_metrics: false,
            model_calls: false,
            agent_orchestration: false,
            build: false,
            errors: false,
            debug_to_stderr: false,
        }
    }

    /// Resolve the active config from env vars.
    ///
    /// Resolution order (highest wins):
    ///   1. `VOX_TELEMETRY` master (`off|on|debug`)
    ///   2. Legacy per-category env vars
    ///   3. Default: local-on, remote-off, all categories on
    pub fn from_env() -> Self {
        let master = std::env::var("VOX_TELEMETRY")
            .ok()
            .map(|v| v.to_ascii_lowercase());
        match master.as_deref() {
            Some("off") | Some("0") | Some("false") => return Self::all_off(),
            _ => {}
        }

        let debug_to_stderr = matches!(master.as_deref(), Some("debug"));
        let benchmark_legacy = env_flag("VOX_BENCHMARK_TELEMETRY");
        let mcp_cost_legacy = env_flag("VOX_MCP_LLM_COST_EVENTS");

        Self {
            enabled: true,
            remote_upload: false,
            research_metrics: benchmark_legacy.unwrap_or(true),
            model_calls: mcp_cost_legacy.unwrap_or(true),
            agent_orchestration: true,
            build: true,
            errors: true,
            debug_to_stderr,
        }
    }

    /// Backward-compatible alias for `from_env`.
    #[inline]
    pub fn from_env_legacy() -> Self {
        Self::from_env()
    }
}

/// Returns true if telemetry is allowed at all (master switch is not "off").
///
/// This is the single check that legacy gates should consult before doing
/// their per-category checks. When this returns false, NO telemetry should
/// be emitted regardless of legacy env vars.
pub fn is_master_enabled() -> bool {
    !matches!(
        std::env::var("VOX_TELEMETRY")
            .ok()
            .map(|v| v.to_ascii_lowercase())
            .as_deref(),
        Some("off") | Some("0") | Some("false")
    )
}

fn env_flag(key: &str) -> Option<bool> {
    match std::env::var(key).ok()?.trim() {
        "1" | "true" | "yes" => Some(true),
        "0" | "false" | "no" => Some(false),
        _ => None,
    }
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self::from_env()
    }
}

#[cfg(test)]
// Rust 2024 made `std::env::set_var` / `remove_var` `unsafe`. Each test below
// is gated by `#[serial]` (single-threaded execution against this crate's env
// surface), which is the SAFETY contract for the env mutation blocks.
#[allow(unsafe_code)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn default_is_local_on_remote_off() {
        unsafe {
            std::env::remove_var("VOX_TELEMETRY");
            std::env::remove_var("VOX_BENCHMARK_TELEMETRY");
            std::env::remove_var("VOX_MCP_LLM_COST_EVENTS");
        }
        let cfg = TelemetryConfig::from_env();
        assert!(cfg.enabled);
        assert!(!cfg.remote_upload);
        assert!(cfg.research_metrics);
        assert!(cfg.model_calls);
        assert!(cfg.build);
        assert!(!cfg.debug_to_stderr);
    }

    #[test]
    #[serial]
    fn debug_mode_sets_debug_to_stderr_and_stays_enabled() {
        unsafe {
            std::env::set_var("VOX_TELEMETRY", "debug");
        }
        let cfg = TelemetryConfig::from_env();
        assert!(cfg.enabled, "debug mode should keep telemetry enabled");
        assert!(cfg.debug_to_stderr, "debug mode should set debug_to_stderr");
        unsafe {
            std::env::remove_var("VOX_TELEMETRY");
        }
    }

    #[test]
    #[serial]
    fn master_off_disables_everything() {
        unsafe {
            std::env::set_var("VOX_TELEMETRY", "off");
        }
        let cfg = TelemetryConfig::from_env();
        assert!(!cfg.enabled);
        assert!(!cfg.research_metrics);
        unsafe {
            std::env::remove_var("VOX_TELEMETRY");
        }
    }

    #[test]
    #[serial]
    fn is_master_enabled_responds_to_master_off() {
        unsafe {
            std::env::set_var("VOX_TELEMETRY", "off");
        }
        assert!(!is_master_enabled());
        unsafe {
            std::env::set_var("VOX_TELEMETRY", "on");
        }
        assert!(is_master_enabled());
        unsafe {
            std::env::remove_var("VOX_TELEMETRY");
        }
        assert!(is_master_enabled()); // unset = default-on
    }
}
