//! Central Oratio runtime tunables with precedence: **explicit overrides → env → config file → defaults**.
//!
//! Config file path: `VOX_ORATIO_CONFIG` (TOML). Env vars use the `VOX_ORATIO_*` prefix documented on each field.

use std::path::Path;
use std::sync::OnceLock;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Tunables for deterministic refinement confidence (mirrors former magic numbers in `refine::rules`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RefineTunables {
    /// Base confidence for `Conservative` profile.
    pub conservative_base: f32,
    /// Base confidence for `Balanced` profile.
    pub balanced_base: f32,
    /// Base confidence for `Aggressive` profile.
    pub aggressive_base: f32,
    /// Penalty per applied trace entry.
    pub penalty_per_trace: f32,
    /// Maximum total penalty from traces.
    pub penalty_cap: f32,
    /// Minimum clamped confidence.
    pub conf_min: f32,
    /// Maximum clamped confidence.
    pub conf_max: f32,
}

impl Default for RefineTunables {
    fn default() -> Self {
        Self {
            conservative_base: 0.82,
            balanced_base: 0.87,
            aggressive_base: 0.9,
            penalty_per_trace: 0.015,
            penalty_cap: 0.25,
            conf_min: 0.1,
            conf_max: 0.99,
        }
    }
}

/// Routing / chat memory guardrails.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoutingTunables {
    /// Minimum transcript confidence before `RouteMode::Tool` may execute.
    pub tool_route_min_confidence: f32,
    /// Max user+assistant lines retained per chat session (after trim).
    pub chat_max_messages: usize,
    /// Max user turns per session id before short-circuit (`no_match` / `guard_max_turns`).
    pub route_max_user_turns: usize,
    /// Minimum confidence before orchestrator route accepts without disambiguation hint.
    pub orchestrator_min_confidence: f32,
}

impl Default for RoutingTunables {
    fn default() -> Self {
        Self {
            tool_route_min_confidence: 0.55,
            chat_max_messages: 64,
            route_max_user_turns: 32,
            orchestrator_min_confidence: 0.45,
        }
    }
}

/// Hugging Face download retry policy.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HfTunables {
    /// HF Hub download retry count (minimum 1 after merge).
    pub retry_attempts: usize,
    /// Delay between retries in milliseconds.
    pub retry_delay_ms: u64,
}

impl Default for HfTunables {
    fn default() -> Self {
        Self {
            retry_attempts: 3,
            retry_delay_ms: 600,
        }
    }
}

/// Optional LLM polish policy (MCP / future CLI). Deterministic refine remains baseline.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LlmPolicyTunables {
    /// Default `llm_refinement` when caller omits flag.
    pub llm_refinement_default: bool,
    /// Minimum deterministic confidence before LLM may be skipped (when enabled).
    pub llm_min_det_confidence: f32,
    /// Max completion tokens budget.
    pub llm_max_output_tokens: u64,
}

impl Default for LlmPolicyTunables {
    fn default() -> Self {
        Self {
            llm_refinement_default: false,
            llm_min_det_confidence: 0.82,
            llm_max_output_tokens: 512,
        }
    }
}

/// Decode-time constrained generation policy (logit bias/masks/trie).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LogitConstraintTunables {
    /// Additive bias for hotword/lexicon token ids.
    pub bias_strength: f32,
    /// Maximum number of unique token ids to bias.
    pub bias_max_tokens: usize,
    /// Enable token-trie hard constraints (`VOX_ORATIO_CONSTRAINED_TRIE` equivalent).
    pub constrained_trie: bool,
    /// Reset trie after this many non-matching decoded tokens.
    pub trie_stuck_steps: usize,
}

impl Default for LogitConstraintTunables {
    fn default() -> Self {
        Self {
            bias_strength: 0.8,
            bias_max_tokens: 256,
            constrained_trie: false,
            trie_stuck_steps: 2,
        }
    }
}

/// Session timing defaults (UX capture vs inference wall-clock caps).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionTimingDefaults {
    /// Enter-or-timeout gates (CLI / MCP contract metadata).
    pub capture_timeout_ms: u64,
    /// Hard cap for whole session wall time (post-inference check today).
    pub max_duration_ms: u64,
    /// Ceiling for transcription/inference segment (post-inference check); `0` = use `max_duration_ms`.
    pub inference_deadline_ms: u64,
    /// Heartbeat log cadence for long waits (CLI / tools).
    pub heartbeat_ms: u64,
}

impl Default for SessionTimingDefaults {
    fn default() -> Self {
        Self {
            capture_timeout_ms: 12_000,
            max_duration_ms: 120_000,
            inference_deadline_ms: 0,
            heartbeat_ms: 3_000,
        }
    }
}

/// Target ASR Backend and Domain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AsrBackendConfig {
    pub backend: String,
    pub domain_mode: crate::refine::DomainMode,
}

impl Default for AsrBackendConfig {
    fn default() -> Self {
        Self {
            backend: "auto".to_string(),
            domain_mode: crate::refine::DomainMode::General,
        }
    }
}

/// Merged Oratio runtime configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct OratioRuntimeConfig {
    /// Target ASR Backend and Domain.
    pub asr: AsrBackendConfig,
    /// Session timing defaults (capture vs inference vs wall cap).
    pub session_timing: SessionTimingDefaults,
    /// Deterministic refine confidence tunables.
    pub refine: RefineTunables,
    /// Routing thresholds and chat memory cap.
    pub routing: RoutingTunables,
    /// Hugging Face artifact fetch policy.
    pub hf: HfTunables,
    /// Default LLM polish policy (MCP; deterministic path remains primary).
    pub llm: LlmPolicyTunables,
    /// Decode-time constrained generation defaults.
    pub logit: LogitConstraintTunables,
}

/// Top-level TOML keys (all optional).
#[derive(Deserialize, Default, Debug)]
struct OratioRuntimeConfigFile {
    capture_timeout_ms: Option<u64>,
    max_duration_ms: Option<u64>,
    inference_deadline_ms: Option<u64>,
    heartbeat_ms: Option<u64>,
    refine_conservative_base: Option<f32>,
    refine_balanced_base: Option<f32>,
    refine_aggressive_base: Option<f32>,
    refine_penalty_per_trace: Option<f32>,
    refine_penalty_cap: Option<f32>,
    refine_confidence_min: Option<f32>,
    refine_confidence_max: Option<f32>,
    tool_route_min_confidence: Option<f32>,
    chat_max_messages: Option<usize>,
    route_max_user_turns: Option<usize>,
    orchestrator_min_confidence: Option<f32>,
    hf_retry_attempts: Option<usize>,
    hf_retry_delay_ms: Option<u64>,
    llm_refinement_default: Option<bool>,
    llm_min_det_confidence: Option<f32>,
    llm_max_output_tokens: Option<u64>,
    logit_bias_strength: Option<f32>,
    logit_bias_max_tokens: Option<usize>,
    logit_constrained_trie: Option<bool>,
    logit_trie_stuck_steps: Option<usize>,
}

impl OratioRuntimeConfig {
    /// Load TOML from path and merge into `self` (file values are lower precedence than prior `self` fields — call before [`merge_env`](Self::merge_env) for env > file).
    pub fn merge_file(&mut self, path: &Path) -> Result<()> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("read Oratio config {}", path.display()))?;
        let flat: OratioRuntimeConfigFile =
            toml::from_str(&raw).context("parse Oratio TOML config")?;
        if let Some(v) = flat.capture_timeout_ms {
            self.session_timing.capture_timeout_ms = v;
        }
        if let Some(v) = flat.max_duration_ms {
            self.session_timing.max_duration_ms = v;
        }
        if let Some(v) = flat.inference_deadline_ms {
            self.session_timing.inference_deadline_ms = v;
        }
        if let Some(v) = flat.heartbeat_ms {
            self.session_timing.heartbeat_ms = v;
        }
        if let Some(v) = flat.refine_conservative_base {
            self.refine.conservative_base = v;
        }
        if let Some(v) = flat.refine_balanced_base {
            self.refine.balanced_base = v;
        }
        if let Some(v) = flat.refine_aggressive_base {
            self.refine.aggressive_base = v;
        }
        if let Some(v) = flat.refine_penalty_per_trace {
            self.refine.penalty_per_trace = v;
        }
        if let Some(v) = flat.refine_penalty_cap {
            self.refine.penalty_cap = v;
        }
        if let Some(v) = flat.refine_confidence_min {
            self.refine.conf_min = v;
        }
        if let Some(v) = flat.refine_confidence_max {
            self.refine.conf_max = v;
        }
        if let Some(v) = flat.tool_route_min_confidence {
            self.routing.tool_route_min_confidence = v;
        }
        if let Some(v) = flat.chat_max_messages {
            self.routing.chat_max_messages = v;
        }
        if let Some(v) = flat.route_max_user_turns {
            self.routing.route_max_user_turns = v;
        }
        if let Some(v) = flat.orchestrator_min_confidence {
            self.routing.orchestrator_min_confidence = v;
        }
        if let Some(v) = flat.hf_retry_attempts {
            self.hf.retry_attempts = v.max(1);
        }
        if let Some(v) = flat.hf_retry_delay_ms {
            self.hf.retry_delay_ms = v;
        }
        if let Some(v) = flat.llm_refinement_default {
            self.llm.llm_refinement_default = v;
        }
        if let Some(v) = flat.llm_min_det_confidence {
            self.llm.llm_min_det_confidence = v;
        }
        if let Some(v) = flat.llm_max_output_tokens {
            self.llm.llm_max_output_tokens = v.max(64);
        }
        if let Some(v) = flat.logit_bias_strength {
            self.logit.bias_strength = v;
        }
        if let Some(v) = flat.logit_bias_max_tokens {
            self.logit.bias_max_tokens = v.max(1);
        }
        if let Some(v) = flat.logit_constrained_trie {
            self.logit.constrained_trie = v;
        }
        if let Some(v) = flat.logit_trie_stuck_steps {
            self.logit.trie_stuck_steps = v.max(1);
        }
        Ok(())
    }

    /// Merge environment variables (override file + defaults).
    pub fn merge_env(&mut self) {
        use std::env::var;
        macro_rules! env_u64 {
            ($key:expr, $field:expr) => {
                if let Ok(s) = var($key) {
                    if let Ok(v) = s.parse::<u64>() {
                        $field = v;
                    }
                }
            };
        }
        macro_rules! env_f32 {
            ($key:expr, $field:expr) => {
                if let Ok(s) = var($key) {
                    if let Ok(v) = s.parse::<f32>() {
                        $field = v;
                    }
                }
            };
        }
        macro_rules! env_bool {
            ($key:expr, $field:expr) => {
                if let Ok(s) = var($key) {
                    if s == "1" || s.eq_ignore_ascii_case("true") {
                        $field = true;
                    } else if s == "0" || s.eq_ignore_ascii_case("false") {
                        $field = false;
                    }
                }
            };
        }

        env_u64!(
            "VOX_ORATIO_CAPTURE_TIMEOUT_MS",
            self.session_timing.capture_timeout_ms
        );
        env_u64!(
            "VOX_ORATIO_MAX_DURATION_MS",
            self.session_timing.max_duration_ms
        );
        env_u64!(
            "VOX_ORATIO_INFERENCE_DEADLINE_MS",
            self.session_timing.inference_deadline_ms
        );
        env_u64!("VOX_ORATIO_HEARTBEAT_MS", self.session_timing.heartbeat_ms);

        env_f32!(
            "VOX_ORATIO_REFINE_CONSERVATIVE_BASE",
            self.refine.conservative_base
        );
        env_f32!("VOX_ORATIO_REFINE_BALANCED_BASE", self.refine.balanced_base);
        env_f32!(
            "VOX_ORATIO_REFINE_AGGRESSIVE_BASE",
            self.refine.aggressive_base
        );
        env_f32!(
            "VOX_ORATIO_REFINE_PENALTY_PER_TRACE",
            self.refine.penalty_per_trace
        );
        env_f32!("VOX_ORATIO_REFINE_PENALTY_CAP", self.refine.penalty_cap);
        env_f32!("VOX_ORATIO_REFINE_CONFIDENCE_MIN", self.refine.conf_min);
        env_f32!("VOX_ORATIO_REFINE_CONFIDENCE_MAX", self.refine.conf_max);

        env_f32!(
            "VOX_ORATIO_TOOL_ROUTE_MIN_CONFIDENCE",
            self.routing.tool_route_min_confidence
        );
        if let Ok(s) = var("VOX_ORATIO_CHAT_MAX_MESSAGES")
            && let Ok(v) = s.parse::<usize>()
            && v >= 4
        {
            self.routing.chat_max_messages = v;
        }
        if let Ok(s) = var("VOX_ORATIO_ROUTE_MAX_USER_TURNS")
            && let Ok(v) = s.parse::<usize>()
            && v >= 1
        {
            self.routing.route_max_user_turns = v;
        }
        env_f32!(
            "VOX_ORATIO_ORCHESTRATOR_MIN_CONFIDENCE",
            self.routing.orchestrator_min_confidence
        );

        if let Ok(s) = var("VOX_ORATIO_HF_RETRY_ATTEMPTS")
            && let Ok(v) = s.parse::<usize>()
        {
            self.hf.retry_attempts = v.max(1);
        }
        env_u64!("VOX_ORATIO_HF_RETRY_DELAY_MS", self.hf.retry_delay_ms);

        env_bool!(
            "VOX_ORATIO_LLM_REFINEMENT_DEFAULT",
            self.llm.llm_refinement_default
        );
        env_f32!(
            "VOX_ORATIO_LLM_MIN_CONFIDENCE",
            self.llm.llm_min_det_confidence
        );
        if let Ok(s) = var("VOX_ORATIO_LLM_MAX_OUTPUT_TOKENS")
            && let Ok(v) = s.parse::<u64>()
        {
            self.llm.llm_max_output_tokens = v.max(64);
        }
        env_f32!("VOX_ORATIO_LOGIT_BIAS_STRENGTH", self.logit.bias_strength);
        if let Ok(s) = var("VOX_ORATIO_LOGIT_BIAS_MAX_TOKENS")
            && let Ok(v) = s.parse::<usize>()
        {
            self.logit.bias_max_tokens = v.max(1);
        }
        env_bool!("VOX_ORATIO_CONSTRAINED_TRIE", self.logit.constrained_trie);
        if let Ok(s) = var("VOX_ORATIO_TRIE_STUCK_STEPS")
            && let Ok(v) = s.parse::<usize>()
        {
            self.logit.trie_stuck_steps = v.max(1);
        }

        if let Ok(s) = var("VOX_ORATIO_BACKEND") {
            self.asr.backend = s.trim().to_ascii_lowercase();
        }
        if let Ok(s) = var("VOX_ORATIO_DOMAIN_MODE") {
            if s.trim().eq_ignore_ascii_case("code") {
                self.asr.domain_mode = crate::refine::DomainMode::Code;
            } else {
                self.asr.domain_mode = crate::refine::DomainMode::General;
            }
        }
    }

    /// `defaults` → optional `VOX_ORATIO_CONFIG` file → env.
    pub fn resolve() -> Self {
        let mut c = Self::default();
        if let Ok(p) = std::env::var("VOX_ORATIO_CONFIG")
            && let Err(e) = c.merge_file(Path::new(&p))
        {
            tracing::warn!(
                target: "vox_oratio_config",
                path = %p,
                error = %e,
                "failed to load Oratio config file"
            );
        }
        c.merge_env();
        c
    }

    /// Effective inference deadline (`inference_deadline_ms` or `max_duration_ms` when zero).
    #[must_use]
    pub fn effective_inference_deadline_ms(&self) -> u64 {
        let d = self.session_timing.inference_deadline_ms;
        if d == 0 {
            self.session_timing.max_duration_ms
        } else {
            d
        }
    }
}

fn cached_resolve_config() -> &'static OratioRuntimeConfig {
    static CACHE: OnceLock<OratioRuntimeConfig> = OnceLock::new();
    CACHE.get_or_init(OratioRuntimeConfig::resolve)
}

/// Cached resolved config for hot paths (`tool_route_min_confidence`, routing).
#[must_use]
pub fn resolved_runtime_config() -> &'static OratioRuntimeConfig {
    cached_resolve_config()
}

/// Minimum deterministic confidence before `RouteMode::Tool` may execute (uses cached [`resolved_runtime_config`]).
#[must_use]
pub fn tool_route_min_confidence() -> f32 {
    let v = resolved_runtime_config().routing.tool_route_min_confidence;
    if (0.0..=1.0).contains(&v) {
        v
    } else {
        RoutingTunables::default().tool_route_min_confidence
    }
}

/// Build a JSON [`serde_json::Value`] describing resolved config for diagnostics (no secrets).
#[must_use]
pub fn runtime_config_diagnostic_json(cfg: &OratioRuntimeConfig) -> serde_json::Value {
    serde_json::json!({
        "session_timing": cfg.session_timing,
        "refine": cfg.refine,
        "routing": cfg.routing,
        "hf": cfg.hf,
        "llm": cfg.llm,
        "logit": cfg.logit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[allow(unsafe_code)]
    #[test]
    fn precedence_env_over_default() {
        let prev = std::env::var("VOX_ORATIO_TOOL_ROUTE_MIN_CONFIDENCE").ok();
        // SAFETY: test-local env mutation; suite is not parallelized with other threads reading this var.
        unsafe {
            std::env::set_var("VOX_ORATIO_TOOL_ROUTE_MIN_CONFIDENCE", "0.71");
        }
        let mut c = OratioRuntimeConfig::default();
        c.merge_env();
        assert!((c.routing.tool_route_min_confidence - 0.71).abs() < 0.001);
        unsafe {
            match prev {
                Some(p) => std::env::set_var("VOX_ORATIO_TOOL_ROUTE_MIN_CONFIDENCE", p),
                None => std::env::remove_var("VOX_ORATIO_TOOL_ROUTE_MIN_CONFIDENCE"),
            }
        }
    }

    #[test]
    fn toml_merge_roundtrip() {
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            tmp,
            r"
capture_timeout_ms = 5000
tool_route_min_confidence = 0.6
refine_balanced_base = 0.88
hf_retry_attempts = 2
"
        )
        .unwrap();
        let mut c = OratioRuntimeConfig::default();
        c.merge_file(tmp.path()).unwrap();
        assert_eq!(c.session_timing.capture_timeout_ms, 5000);
        assert!((c.routing.tool_route_min_confidence - 0.6).abs() < 0.001);
        assert!((c.refine.balanced_base - 0.88).abs() < 0.001);
        assert_eq!(c.hf.retry_attempts, 2);
    }
}
