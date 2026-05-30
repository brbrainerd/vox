//! Configuration for `vox audit effort`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EffortAuditConfig {
    #[serde(default = "default_since")]
    pub default_since: String,
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: usize,
    #[serde(default = "default_max_diff_bytes")]
    pub max_diff_bytes: usize,
    #[serde(default = "default_true")]
    pub with_transcripts: bool,
    #[serde(default = "default_transcript_dir")]
    pub transcript_dir: PathBuf,
    #[serde(default = "default_report_top_n")]
    pub report_top_n: usize,
    #[serde(default)]
    pub judge: JudgeConfig,
    /// Optional hard cap on the number of commits judged. `None` = no cap.
    ///
    /// Threaded from `vox audit effort --limit N` (F1) so CI smoke runs and
    /// dry-runs can bound LLM spend without changing `default_since`. The
    /// limiter is enforced inside the pipeline's dispatch loop *after* the
    /// walker yields its full set, so range-based statistics (e.g.
    /// `commits_in_range` in the manifest) still reflect the true window.
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JudgeConfig {
    pub model_preference: Option<String>,
    #[serde(default = "default_max_total_tokens")]
    pub max_total_tokens: u64,
    #[serde(default = "default_max_dollar_cost")]
    pub max_dollar_cost: f64,
    #[serde(default = "default_schema_retry_limit")]
    pub schema_retry_limit: u32,
}

impl Default for JudgeConfig {
    fn default() -> Self {
        Self {
            model_preference: None,
            max_total_tokens: default_max_total_tokens(),
            max_dollar_cost: default_max_dollar_cost(),
            schema_retry_limit: default_schema_retry_limit(),
        }
    }
}

fn default_since() -> String {
    "30 days ago".into()
}
fn default_max_concurrent() -> usize {
    4
}
fn default_max_diff_bytes() -> usize {
    200 * 1024
}
fn default_true() -> bool {
    true
}
fn default_transcript_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(".claude/projects")
}
fn default_report_top_n() -> usize {
    20
}
fn default_max_total_tokens() -> u64 {
    5_000_000
}
fn default_max_dollar_cost() -> f64 {
    5.00
}
fn default_schema_retry_limit() -> u32 {
    1
}

impl Default for EffortAuditConfig {
    fn default() -> Self {
        Self {
            default_since: default_since(),
            max_concurrent: default_max_concurrent(),
            max_diff_bytes: default_max_diff_bytes(),
            with_transcripts: default_true(),
            transcript_dir: default_transcript_dir(),
            report_top_n: default_report_top_n(),
            judge: JudgeConfig::default(),
            limit: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec() {
        let c = EffortAuditConfig::default();
        assert_eq!(c.default_since, "30 days ago");
        assert_eq!(c.max_concurrent, 4);
        assert_eq!(c.max_diff_bytes, 200 * 1024);
        assert!(c.with_transcripts);
        assert_eq!(c.report_top_n, 20);
        assert_eq!(c.judge.max_total_tokens, 5_000_000);
        assert!((c.judge.max_dollar_cost - 5.00).abs() < f64::EPSILON);
    }

    #[test]
    fn defaults_have_no_limit() {
        // `limit` defaults to None so the CLI `--limit` flag is purely
        // additive — no behavior change for callers who don't supply it.
        assert!(EffortAuditConfig::default().limit.is_none());
    }

    #[test]
    fn limit_round_trips_through_toml() {
        let t: EffortAuditConfig = toml::from_str("limit = 5\n").unwrap();
        assert_eq!(t.limit, Some(5));
    }

    #[test]
    fn partial_toml_inherits_defaults() {
        let t: EffortAuditConfig = toml::from_str(
            r#"
            default_since = "7 days ago"
            [judge]
            model_preference = "mens-r6.2"
        "#,
        )
        .unwrap();
        assert_eq!(t.default_since, "7 days ago");
        assert_eq!(t.judge.model_preference.as_deref(), Some("mens-r6.2"));
        assert_eq!(t.max_concurrent, 4); // default
    }
}
