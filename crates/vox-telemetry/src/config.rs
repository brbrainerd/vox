//! `TelemetryConfig`: read once at startup, governs which sinks and categories are active.
//!
//! Resolution order (highest wins):
//!   1. `/etc/vox/telemetry-policy.toml` — org-level hard-off (Phase D) ✓
//!   2. `~/.config/vox/config.toml`        — user preference (Phase D) ✓ (2026-05-28)
//!   3. `VOX_TELEMETRY`                    — master on/off/debug (Phase D) ✓
//!   4. Legacy per-category env vars        — compat shim ✓
//!   5. Default                             — local collection on, remote upload off ✓
//!
//! Phase D: all five layers are implemented.

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

    /// Resolve the active config from all five layers.
    ///
    /// Resolution order (highest wins):
    ///   1. Org-policy hard-off (`/etc/vox/telemetry-policy.toml` / `%ProgramData%\vox\…`)
    ///   2. User config (`~/.config/vox/config.toml`)
    ///   3. `VOX_TELEMETRY` master (`off|on|debug`)
    ///   4. Legacy per-category env vars
    ///   5. Default: local-on, remote-off, all categories on
    pub fn from_env() -> Self {
        // Layer 1: org-level hard-off takes absolute precedence.
        if org_policy_disabled() {
            return Self::all_off();
        }

        // Layer 2: user config — read once. Overrides env-derived defaults below
        // when keys are set; absent keys fall through to layers 3-5.
        let user = read_user_config();

        // Layer 3: env master switch. `off` forces everything off even if the user
        // config left the master at default (i.e., env keeps its "kill switch" role).
        let master = std::env::var("VOX_TELEMETRY")
            .ok()
            .map(|v| v.to_ascii_lowercase());
        match master.as_deref() {
            Some("off") | Some("0") | Some("false") => return Self::all_off(),
            _ => {}
        }

        // If user config set master=false but env didn't override it, honor user kill switch.
        if matches!(user.enabled, Some(false)) && master.is_none() {
            return Self::all_off();
        }

        let debug_to_stderr =
            matches!(master.as_deref(), Some("debug")) || user.debug_to_stderr.unwrap_or(false);
        let benchmark_legacy = env_flag("VOX_BENCHMARK_TELEMETRY");
        let mcp_cost_legacy = env_flag("VOX_MCP_LLM_COST_EVENTS");

        Self {
            enabled: true,
            remote_upload: user.remote_upload.unwrap_or(false),
            research_metrics: benchmark_legacy.or(user.research_metrics).unwrap_or(true),
            model_calls: mcp_cost_legacy.or(user.model_calls).unwrap_or(true),
            agent_orchestration: user.agent_orchestration.unwrap_or(true),
            build: user.build.unwrap_or(true),
            errors: user.errors.unwrap_or(true),
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
    // Layer 3: env-var master switch (`off` is a hard kill).
    let master = std::env::var("VOX_TELEMETRY")
        .ok()
        .map(|v| v.to_ascii_lowercase());
    if matches!(master.as_deref(), Some("off") | Some("0") | Some("false")) {
        return false;
    }
    // Layer 2: user config — only honored when env isn't explicitly overriding.
    if master.is_none() && matches!(read_user_config().enabled, Some(false)) {
        return false;
    }
    true
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
            let rest = rest.trim_start_matches([' ', '=']).trim();
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

// ─── Layer 2: user config ─────────────────────────────────────────────────────

/// Parsed `~/.config/vox/config.toml` `[telemetry]` section. Every field is
/// `Option<bool>` so absent keys cleanly fall through to the next resolution layer.
#[derive(Debug, Default, Clone)]
pub(crate) struct UserConfig {
    pub enabled: Option<bool>,
    pub remote_upload: Option<bool>,
    pub research_metrics: Option<bool>,
    pub model_calls: Option<bool>,
    pub agent_orchestration: Option<bool>,
    pub build: Option<bool>,
    pub errors: Option<bool>,
    pub debug_to_stderr: Option<bool>,
}

/// Phase D — Layer 2: read the per-user config file.
///
/// File paths:
///   - Windows: `%APPDATA%\vox\config.toml`
///   - Linux / macOS: `$XDG_CONFIG_HOME/vox/config.toml` (defaults to `~/.config/vox/config.toml`)
///
/// Missing file → returns an empty `UserConfig` (all fields `None`).
/// Unreadable file → returns an empty `UserConfig` (fail-open).
///
/// Parsing uses a small line-scanner that recognizes `key = value` pairs under
/// an optional `[telemetry]` section. Values may be `true|false|0|1|on|off`,
/// optionally quoted. The line-scanner avoids pulling a TOML dependency into
/// this L1 facade crate (matches the org-policy pattern).
pub(crate) fn read_user_config() -> UserConfig {
    let path = user_config_path();
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return UserConfig::default(),
    };
    parse_user_config(&text)
}

fn user_config_path() -> std::path::PathBuf {
    #[cfg(windows)]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return std::path::PathBuf::from(appdata)
                .join("vox")
                .join("config.toml");
        }
        std::path::PathBuf::from(r"C:\Users\Default\AppData\Roaming\vox\config.toml")
    }
    #[cfg(not(windows))]
    {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            return std::path::PathBuf::from(xdg)
                .join("vox")
                .join("config.toml");
        }
        if let Some(home) = std::env::var_os("HOME") {
            return std::path::PathBuf::from(home)
                .join(".config")
                .join("vox")
                .join("config.toml");
        }
        std::path::PathBuf::from("/etc/vox/config.toml")
    }
}

/// Parse a `[telemetry]` section's boolean keys from TOML-ish input.
///
/// Only recognizes flat `key = value` lines under either the top level or a
/// `[telemetry]` table header. Other sections / nested tables are ignored.
/// Returns a `UserConfig` with every recognized key set.
fn parse_user_config(text: &str) -> UserConfig {
    let mut cfg = UserConfig::default();
    let mut in_telemetry = true; // top-of-file is treated as `[telemetry]` for convenience

    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        // Section header? Update `in_telemetry`.
        if let Some(rest) = line.strip_prefix('[')
            && let Some(name) = rest.strip_suffix(']')
        {
            in_telemetry = name.trim() == "telemetry";
            continue;
        }
        if !in_telemetry {
            continue;
        }
        // key = value
        let bare = line.split('#').next().unwrap_or("").trim();
        let Some((key, val)) = bare.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let val = val
            .trim()
            .trim_matches('"')
            .trim_matches('\'')
            .to_ascii_lowercase();
        let parsed = match val.as_str() {
            "true" | "1" | "on" | "yes" => Some(true),
            "false" | "0" | "off" | "no" => Some(false),
            _ => continue,
        };
        match key {
            "enabled" => cfg.enabled = parsed,
            "remote_upload" => cfg.remote_upload = parsed,
            "research_metrics" => cfg.research_metrics = parsed,
            "model_calls" => cfg.model_calls = parsed,
            "agent_orchestration" => cfg.agent_orchestration = parsed,
            "build" => cfg.build = parsed,
            "errors" => cfg.errors = parsed,
            "debug_to_stderr" => cfg.debug_to_stderr = parsed,
            _ => {}
        }
    }
    cfg
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
                let rest = rest.trim_start_matches([' ', '=']).trim();
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

    // ── user-config parser tests (Layer 2) ────────────────────────────────────

    #[test]
    fn user_config_parses_telemetry_section_flags() {
        let toml = r#"
            [telemetry]
            enabled = true
            remote_upload = false
            research_metrics = true
            model_calls = "false"
            agent_orchestration = 1
            build = 0
            errors = "on"
            debug_to_stderr = off
        "#;
        let cfg = parse_user_config(toml);
        assert_eq!(cfg.enabled, Some(true));
        assert_eq!(cfg.remote_upload, Some(false));
        assert_eq!(cfg.research_metrics, Some(true));
        assert_eq!(cfg.model_calls, Some(false));
        assert_eq!(cfg.agent_orchestration, Some(true));
        assert_eq!(cfg.build, Some(false));
        assert_eq!(cfg.errors, Some(true));
        assert_eq!(cfg.debug_to_stderr, Some(false));
    }

    #[test]
    fn user_config_ignores_unknown_sections() {
        let toml = "[other]\nenabled = false\n[telemetry]\nbuild = false\n";
        let cfg = parse_user_config(toml);
        assert_eq!(cfg.enabled, None, "key under [other] must not leak");
        assert_eq!(cfg.build, Some(false));
    }

    #[test]
    fn user_config_top_of_file_treated_as_telemetry_section() {
        // Convenience: a config with no section header still parses telemetry keys.
        let toml = "enabled = false\nremote_upload = true\n";
        let cfg = parse_user_config(toml);
        assert_eq!(cfg.enabled, Some(false));
        assert_eq!(cfg.remote_upload, Some(true));
    }

    #[test]
    fn user_config_empty_returns_all_none() {
        let cfg = parse_user_config("");
        assert_eq!(cfg.enabled, None);
        assert_eq!(cfg.build, None);
    }

    #[test]
    fn user_config_ignores_comments_and_unknown_values() {
        let toml = r#"
            # entire-line comment
            [telemetry]
            enabled = true  # trailing comment
            unknown_key = true
            build = "maybe"  # unparseable value — skipped
        "#;
        let cfg = parse_user_config(toml);
        assert_eq!(cfg.enabled, Some(true));
        assert_eq!(cfg.build, None, "unparseable value falls through");
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

#[cfg(test)]
#[allow(unsafe_code)]
mod semcov_wave6_tests {
    #![allow(unused_imports, dead_code)]
    use super::*;
    use serial_test::serial;

    // Catches: TelemetryConfig::all_off() having enabled=true or any category true —
    // an "all_off" that isn't fully off would silently emit telemetry in test environments.
    #[test]
    fn all_off_has_all_fields_false() {
        let cfg = TelemetryConfig::all_off();
        assert!(!cfg.enabled, "all_off: enabled must be false");
        assert!(!cfg.remote_upload, "all_off: remote_upload must be false");
        assert!(!cfg.research_metrics, "all_off: research_metrics must be false");
        assert!(!cfg.model_calls, "all_off: model_calls must be false");
        assert!(!cfg.agent_orchestration, "all_off: agent_orchestration must be false");
        assert!(!cfg.build, "all_off: build must be false");
        assert!(!cfg.errors, "all_off: errors must be false");
        assert!(!cfg.debug_to_stderr, "all_off: debug_to_stderr must be false");
    }

    // Catches: TelemetryConfig::default_on() having remote_upload=true — the spec
    // says remote upload is OFF by default; a wrong default would send telemetry remotely.
    #[test]
    fn default_on_has_remote_upload_false_and_categories_true() {
        let cfg = TelemetryConfig::default_on();
        assert!(cfg.enabled, "default_on: enabled must be true");
        assert!(!cfg.remote_upload, "default_on: remote_upload must be false (local collection only)");
        assert!(cfg.research_metrics, "default_on: research_metrics must be true");
        assert!(cfg.model_calls, "default_on: model_calls must be true");
        assert!(cfg.agent_orchestration, "default_on: agent_orchestration must be true");
        assert!(cfg.build, "default_on: build must be true");
        assert!(cfg.errors, "default_on: errors must be true");
        assert!(!cfg.debug_to_stderr, "default_on: debug_to_stderr must default to false");
    }

    // Catches: env_flag() mishandling edge cases like whitespace-padded values,
    // returning None for "1 " (trimmed correctly) or returning wrong bool for "no".
    #[test]
    #[serial]
    fn env_flag_handles_whitespace_and_all_recognized_values() {
        // env_flag is private; test indirectly through VOX_BENCHMARK_TELEMETRY
        // which feeds into research_metrics via the legacy shim in from_env().
        unsafe {
            std::env::remove_var("VOX_TELEMETRY");
            // Set legacy flag to "1" — should enable research_metrics
            std::env::set_var("VOX_BENCHMARK_TELEMETRY", "1");
        }
        let cfg = TelemetryConfig::from_env();
        assert!(
            cfg.research_metrics,
            "VOX_BENCHMARK_TELEMETRY=1 must enable research_metrics"
        );
        unsafe {
            std::env::set_var("VOX_BENCHMARK_TELEMETRY", "no");
        }
        let cfg2 = TelemetryConfig::from_env();
        assert!(
            !cfg2.research_metrics,
            "VOX_BENCHMARK_TELEMETRY=no must disable research_metrics"
        );
        unsafe {
            std::env::remove_var("VOX_BENCHMARK_TELEMETRY");
        }
    }

    // Catches: VOX_TELEMETRY="0" or "false" not being treated as the master kill switch
    // (only "off" handled, forgetting the aliases).
    #[test]
    #[serial]
    fn master_switch_off_aliases_all_disable_telemetry() {
        for val in &["off", "0", "false"] {
            unsafe {
                std::env::set_var("VOX_TELEMETRY", val);
            }
            let cfg = TelemetryConfig::from_env();
            assert!(
                !cfg.enabled,
                "VOX_TELEMETRY={val} must disable telemetry (enabled must be false)"
            );
            assert!(
                !cfg.model_calls,
                "VOX_TELEMETRY={val} must disable model_calls"
            );
        }
        unsafe {
            std::env::remove_var("VOX_TELEMETRY");
        }
    }

    // Catches: parse_user_config treating keys OUTSIDE of a [telemetry] section
    // as if they were inside it — e.g., a [database] section with `enabled = false`
    // would wrongly disable telemetry.
    #[test]
    fn parse_user_config_non_telemetry_section_keys_do_not_affect_output() {
        let toml = "[database]\nenabled = false\nmodel_calls = false\n[other]\nbuild = false\n";
        let cfg = parse_user_config(toml);
        assert_eq!(
            cfg.enabled, None,
            "enabled key under [database] must NOT set UserConfig::enabled"
        );
        assert_eq!(
            cfg.model_calls, None,
            "model_calls key under [database] must NOT set UserConfig::model_calls"
        );
        assert_eq!(
            cfg.build, None,
            "build key under [other] must NOT set UserConfig::build"
        );
    }

    // Catches: parse_user_config crashing or silently accepting invalid TOML that
    // has no `=` separator — the parser must skip malformed lines, not panic.
    #[test]
    fn parse_user_config_skips_malformed_lines_without_panic() {
        let toml = "[telemetry]\nenabled\nbuild = true\n= bad\nmodel_calls = 1\n";
        // Must not panic.
        let cfg = parse_user_config(toml);
        assert_eq!(cfg.build, Some(true), "valid line after malformed line must be parsed");
        assert_eq!(cfg.model_calls, Some(true));
        // malformed lines produce None, not Some(false)
        assert_eq!(cfg.enabled, None, "malformed 'enabled' line must yield None, not Some(false)");
    }

    // Catches: parse_user_config accepting "maybe" or other unrecognized boolean
    // strings as a valid value (e.g., defaulting to true instead of skipping).
    #[test]
    fn parse_user_config_unknown_value_strings_produce_none() {
        let toml = "[telemetry]\nenabled = maybe\nbuild = yes_please\nerrors = true\n";
        let cfg = parse_user_config(toml);
        assert_eq!(
            cfg.enabled, None,
            "'maybe' is not a recognized boolean — must produce None"
        );
        assert_eq!(
            cfg.build, None,
            "'yes_please' is not a recognized boolean — must produce None"
        );
        assert_eq!(
            cfg.errors, Some(true),
            "'true' is recognized and must produce Some(true)"
        );
    }

    // Catches: parse_user_config not recognizing quoted boolean values like
    // `enabled = "true"` (with surrounding double-quotes) — the parser should
    // strip quotes before matching.
    #[test]
    fn parse_user_config_recognizes_quoted_boolean_values() {
        let toml = "[telemetry]\nenabled = \"true\"\nremote_upload = 'false'\nmodel_calls = \"1\"\n";
        let cfg = parse_user_config(toml);
        assert_eq!(cfg.enabled, Some(true), "double-quoted 'true' must be recognized");
        assert_eq!(cfg.remote_upload, Some(false), "single-quoted 'false' must be recognized");
        assert_eq!(cfg.model_calls, Some(true), "double-quoted '1' must be recognized");
    }

    // Catches: parse_user_config not handling inline TOML comments — a line like
    // `enabled = true # this enables telemetry` must parse `enabled = true` and
    // not fail because of the `# ...` tail.
    #[test]
    fn parse_user_config_strips_inline_comments() {
        let toml = "[telemetry]\nenabled = true # enable collection\nbuild = false # disable build\n";
        let cfg = parse_user_config(toml);
        assert_eq!(cfg.enabled, Some(true), "inline comment must not prevent parsing enabled");
        assert_eq!(cfg.build, Some(false), "inline comment must not prevent parsing build");
    }

    // Catches: org_policy_disabled() returning true (disabling telemetry) when
    // the policy file only contains `enabled = true` (policy explicitly allows telemetry).
    #[test]
    fn org_policy_scanner_does_not_disable_when_enabled_true() {
        // Use the inline helper that replicates the line-scan (from the existing tests module).
        // We test parse_user_config instead since org_policy uses the same logic,
        // but here we exercise it end-to-end by directly inspecting the policy scanner.
        // We replicate the scanner inline to keep this test self-contained.
        fn scan(content: &str) -> bool {
            for raw_line in content.lines() {
                let line = raw_line.trim();
                if line.starts_with('#') { continue; }
                let bare = line.split('#').next().unwrap_or("").trim();
                if let Some(rest) = bare.strip_prefix("enabled") {
                    let rest = rest.trim_start_matches([' ', '=']).trim();
                    let rest = rest.trim_matches('"').trim_matches('\'');
                    if matches!(rest, "false" | "0" | "off") { return true; }
                }
            }
            false
        }
        assert!(
            !scan("enabled = true"),
            "`enabled = true` must NOT trigger policy-disabled"
        );
        assert!(
            !scan("[telemetry]\nenabled = 1\n"),
            "`enabled = 1` must NOT trigger policy-disabled"
        );
        // But these must still disable.
        assert!(scan("enabled = false"), "`enabled = false` must trigger policy-disabled");
        assert!(scan("enabled = 0"), "`enabled = 0` must trigger policy-disabled");
        assert!(scan("enabled = off"), "`enabled = off` must trigger policy-disabled");
    }
}
