//! Structured eval-run recording (RLHF / dogfood tooling).

/// Parameters for `vox_db::VoxDb::record_eval_run`.
#[derive(Debug, Clone)]
pub struct EvalRunParams<'a> {
    /// Stable id for this eval run (e.g. UUID or batch key).
    pub eval_id: &'a str,
    /// Model artifact path or name, if recorded.
    pub model_path: Option<&'a str>,
    /// Optional structured metric: output format validity.
    pub format_validity: Option<f64>,
    /// Optional fraction of prompts rejected for safety.
    pub safety_rejection_rate: Option<f64>,
    /// Optional proxy score for “quality” when gold labels are absent.
    pub quality_proxy: Option<f64>,
    /// Optional metric tracking the number of skills discovered dynamically.
    pub skills_discovered: Option<i64>,
    /// Optional metric tracking the number of workflows discovered dynamically.
    pub workflows_discovered: Option<i64>,
    /// Optional JSON blob for extra columns / tooling.
    pub metadata_json: Option<&'a str>,
}
