//! Eval gate policy types and YAML loading.

use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

use vox_bounded_fs::read_utf8_path_capped;

/// Eval gate policy schema (from eval-gates.yaml).
#[derive(Debug, Clone, Deserialize)]
pub struct EvalGatePolicy {
    #[serde(default)]
    #[allow(dead_code)]
    pub version: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub perplexity: PerplexityGate,
    #[serde(default)]
    pub supervised_ratio: SupervisedRatioGate,
    #[serde(default)]
    pub truncation: TruncationGate,
    #[serde(default)]
    pub throughput: ThroughputGate,
    #[serde(default)]
    pub eval_local: EvalLocalGate,
    /// Per-context thresholds keyed by context name ("target", "meta").
    #[serde(default)]
    pub per_context: std::collections::HashMap<String, ContextGateEntry>,
    /// Modal mix gate — warns/blocks when a modality exceeds its ceiling.
    #[serde(default)]
    pub modal_mix: ModalMixGate,
    /// MCP `call_tool` JSON Schema KPI (`tool_arg_schema_kpi` snapshot written next to run artifacts).
    #[serde(default)]
    pub mcp_tool_schema: McpToolSchemaGate,
    /// pass@k gate from `benchmark_passatk.json`.
    #[serde(default)]
    pub pass_at_k: PassAtKGate,
    /// Anti-stub thresholds from eval-local report.
    #[serde(default)]
    pub anti_stub: AntiStubGate,
    /// Review-derived recurrence gate from `review_metrics.json`.
    #[serde(default)]
    pub review_recurrence: ReviewRecurrenceGate,
    /// Rust spoke compilation rate gate.
    #[serde(default)]
    pub rust_compile_rate: RustCompileRateGate,
    /// Clippy clean rate gate.
    #[serde(default)]
    pub clippy_clean_rate: ClippyCleanRateGate,
}

/// Gate on optional `mcp_tool_schema_kpi.json` in the run directory (from `vox-mcp` diagnostics).
#[derive(Debug, Clone, Deserialize, Default)]
pub struct McpToolSchemaGate {
    /// Minimum `strict_validity_rate` (0.0–1.0). `0.0` disables the gate unless `block` is true.
    #[serde(default)]
    pub min_strict_validity_rate: f64,
    #[serde(default)]
    pub block: bool,
    /// Basename only (path segments with `..` are ignored; falls back to default file).
    #[serde(default = "default_mcp_tool_schema_metrics_file")]
    pub metrics_file: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PassAtKGate {
    /// Minimum pass@1 required (0.0–1.0).
    #[serde(default)]
    pub min_pass_rate_at_1: f64,
    /// Minimum pass@k required (0.0–1.0).
    #[serde(default)]
    pub min_pass_rate_at_k: f64,
    /// Maximum tolerated regression vs baseline file.
    #[serde(default = "default_passatk_max_drop")]
    pub max_regression_drop: f64,
    #[serde(default)]
    pub block: bool,
    /// Basename only, default `benchmark_passatk.json`.
    #[serde(default = "default_passatk_metrics_file")]
    pub metrics_file: String,
    /// Optional baseline file in run dir for regression checks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub baseline_file: Option<String>,
}

impl Default for PassAtKGate {
    fn default() -> Self {
        Self {
            min_pass_rate_at_1: 0.0,
            min_pass_rate_at_k: 0.0,
            max_regression_drop: default_passatk_max_drop(),
            block: false,
            metrics_file: default_passatk_metrics_file(),
            baseline_file: None,
        }
    }
}

fn default_passatk_metrics_file() -> String {
    "benchmark_passatk.json".to_string()
}

fn default_passatk_max_drop() -> f64 {
    0.03
}

fn default_mcp_tool_schema_metrics_file() -> String {
    "mcp_tool_schema_kpi.json".to_string()
}

pub(super) fn mcp_tool_schema_metrics_path(run_dir: &Path, configured: &str) -> std::path::PathBuf {
    let p = std::path::Path::new(configured);
    let fname = p.file_name().unwrap_or_default();
    if fname.is_empty() {
        run_dir.join(default_mcp_tool_schema_metrics_file())
    } else {
        run_dir.join(fname)
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PerplexityGate {
    #[serde(default = "default_perplexity_regression")]
    #[allow(dead_code)]
    pub max_regression_ratio: f64,
    #[serde(default)]
    #[allow(dead_code)]
    pub block: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SupervisedRatioGate {
    #[serde(default = "default_min_supervised")]
    pub min_pct: f64,
    #[serde(default)]
    pub block: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TruncationGate {
    #[serde(default = "default_max_truncation")]
    pub max_fraction: f64,
    #[serde(default)]
    pub block: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ThroughputGate {
    #[serde(default)]
    pub min_tokens_per_sec: std::collections::HashMap<String, f64>,
    #[serde(default)]
    pub block: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct EvalLocalGate {
    #[serde(default = "default_min_parse_rate")]
    pub min_parse_rate: f64,
    #[serde(default = "default_min_coverage")]
    pub min_coverage_pct: f64,
    #[serde(default)]
    pub block: bool,
}

fn default_perplexity_regression() -> f64 {
    1.05
}
fn default_min_supervised() -> f64 {
    0.10
}
fn default_max_truncation() -> f64 {
    0.50
}
fn default_min_parse_rate() -> f64 {
    0.60
}
fn default_min_coverage() -> f64 {
    0.35
}

/// Single per-context threshold entry from the `per_context:` map.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ContextGateEntry {
    /// Minimum fraction of records that must parse as valid Vox (0.0–1.0).
    #[serde(default)]
    pub min_parse_rate: f64,
    /// Minimum scope compliance rate (0.0–1.0).
    #[serde(default)]
    pub min_scope_compliance_rate: f64,
    /// If true, a violation blocks the run.
    #[serde(default)]
    pub block: bool,
}

/// Modal-mix gate: warns/blocks when a modality's record fraction exceeds a ceiling.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ModalMixGate {
    /// Maximum fraction of output records that may be tagged `modal: voice` (0.0–1.0).
    #[serde(default)]
    pub max_voice_fraction: f64,
    /// If true, a violation blocks the run.
    #[serde(default)]
    pub block: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct AntiStubGate {
    /// Minimum anti-stub task success rate from `eval_local_report.json`
    /// (written by `vox mens eval-local`; checked by check_run.rs).
    #[serde(default)]
    pub min_pass_rate: f64,
    /// Maximum placeholder marker event rate tolerated.
    #[serde(default)]
    pub max_placeholder_event_rate: f64,
    /// Maximum trivial placeholder event rate tolerated.
    #[serde(default)]
    pub max_trivial_placeholder_event_rate: f64,
    /// Minimum construct richness mean expected from eval-local.
    #[serde(default)]
    pub min_construct_richness_mean: f64,
    /// If true, violations block the run.
    #[serde(default)]
    pub block: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ReviewRecurrenceGate {
    /// Maximum tolerated repeated finding rate (0.0-1.0).
    #[serde(default)]
    pub max_repeated_finding_rate: f64,
    /// Maximum tolerated post-training regression rate (0.0-1.0).
    #[serde(default)]
    pub max_post_training_regression_rate: f64,
    /// Minimum expected recurrence delta improvement (baseline - current).
    #[serde(default)]
    pub min_recurrence_delta: f64,
    /// If true, violations block promotion.
    #[serde(default)]
    pub block: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RustCompileRateGate {
    #[serde(default)]
    pub min_pct: f64,
    #[serde(default)]
    pub block: bool,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ClippyCleanRateGate {
    #[serde(default)]
    pub min_pct: f64,
    #[serde(default)]
    pub block: bool,
}

/// Load policy from path (default: mens/config/eval-gates.yaml).
pub fn load_policy(path: &Path) -> Result<EvalGatePolicy> {
    let content = read_utf8_path_capped(path)?;
    let policy: EvalGatePolicy = serde_yaml::from_str(&content)?;
    Ok(policy)
}
