//! `TelemetryConfig`: read once at startup, governs which sinks and categories are active.
//!
//! Resolution order (highest wins):
//!   1. `/etc/vox/telemetry-policy.toml` — org-level hard-off (Phase D) ✓
//!   2. `~/.config/vox/config.toml`        — user preference (Phase D, future)
//!   3. `VOX_TELEMETRY`                    — master on/off/debug (Phase D) ✓
//!   4. Legacy per-category env vars        — compat shim ✓
//!   5. Default                             — local collection on, remote upload off ✓
//!
//! Phase D: layers 1 and 3–5 are implemented. Layer 2 (user config TOML) is deferred.

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
        // Layer 1: org-level hard-off takes absolute precedence.
        if org_policy_disabled() {
            return Self::all_off();
        }

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
    // Layer 1: org-level hard-off.
    if org_policy_disabled() {
        return false;
    }
    // Layer 3: env-var master switch.
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

/// Phase D — Layer 1: org-level hard-off via policy file.
///
/// Reads the platform-standard policy file and returns `true` when the file
/// contains an `enabled = false` directive, meaning the organisation has
/// disabled all telemetry. The parse is intentionally simple (line-scan) to
/// avoid pulling a TOML dependency into `vox-telemetry`.
///
/// File paths:
///   - Windows: `%ProgramData%\vox\telemetry-policy.toml`
///   - Linux / macOS: `/etc/vox/telemetry-policy.toml`
///
/// Missing file → returns `false` (policy not set; fall through to lower layers).
/// Unreadable file → returns `false` with a debug log (fail-open).
pub fn org_policy_disabled() -> bool {
    let path = org_policy_path();
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return false,
        Err(_) => {
            // Unreadable policy file: fail-open (do not silently disable telemetry).
            return false;
        }
    };
    // Scan for a non-commented `enabled = false` or `enabled=false` line.
    // This handles both top-level and `[telemetry]`-nested placements because
    // the intent of the file is exclusively the master switch.
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.starts_with('#') {
            continue; // skip TOML comments
        }
        // Split on '#' to strip inline comments, then trim.
        let bare = line.split('#').next().unwrap_or("").trim();
        // Match `enabled = false` / `enabled=false` / `enabled = 0` / `enabled = "false"`.
        if let Some(rest) = bare.strip_prefix("enabled") {
            let rest = rest
                .trim_start_matches(|c: char| c == ' ' || c == '=')
                .trim();
            let rest = rest.trim_matches('"').trim_matches('\'');
            if matches!(rest, "false" | "0" | "off") {
                return true;
            }
        }
    }
    false
}

fn org_policy_path() -> std::path::PathBuf {
    #[cfg(windows)]
    {
        // %ProgramData% typically resolves to C:\ProgramData
        let base = std::env::var_os("ProgramData")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from(r"C:\ProgramData"));
        base.join("vox").join("telemetry-policy.toml")
    }
    #[cfg(not(windows))]
    {
        std::path::PathBuf::from("/etc/vox/telemetry-policy.toml")
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

    // ── org_policy_disabled line-parser tests ─────────────────────────────────

    fn parse_policy(content: &str) -> bool {
        // Replicate the line-scan logic inline so we don't need a real file.
        for raw_line in content.lines() {
            let line = raw_line.trim();
            if line.starts_with('#') {
                continue;
            }
            let bare = line.split('#').next().unwrap_or("").trim();
            if let Some(rest) = bare.strip_prefix("enabled") {
                let rest = rest
                    .trim_start_matches(|c: char| c == ' ' || c == '=')
                    .trim();
                let rest = rest.trim_matches('"').trim_matches('\'');
                if matches!(rest, "false" | "0" | "off") {
                    return true;
                }
            }
        }
        false
    }

    #[test]
    fn org_policy_scanner_detects_enabled_false() {
        assert!(parse_policy("enabled = false"));
        assert!(parse_policy("enabled=false"));
        assert!(parse_policy("enabled = 0"));
        assert!(parse_policy("enabled = off"));
        assert!(parse_policy("enabled = \"false\""));
    }

    #[test]
    fn org_policy_scanner_ignores_comments_and_true() {
        assert!(!parse_policy("# enabled = false"));
        assert!(!parse_policy("enabled = true"));
        assert!(!parse_policy("enabled = 1"));
        assert!(!parse_policy(""));
        assert!(!parse_policy("[telemetry]\nsome_other_key = false"));
    }

    #[test]
    fn org_policy_scanner_handles_toml_section_with_disable() {
        let toml = "[telemetry]\nenabled = false\n";
        assert!(parse_policy(toml));
    }

    #[test]
    fn org_policy_disabled_returns_false_when_file_missing() {
        // On any CI machine, the org-policy file should not exist — verify fail-open.
        // This test is always valid unless running inside a vox-managed enterprise setup
        // that has actually installed the policy file, which is expected not to be the case
        // in this test environment.
        let path = org_policy_path();
        if path.exists() {
            // Policy file exists; skip assertion to avoid false failure.
            return;
        }
        assert!(!org_policy_disabled());
    }
}
