//! Configuration for `vox audit effort-route`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EffortRouteConfig {
    #[serde(default = "default_min_waste_score")]
    pub min_waste_score: u8,
    #[serde(default = "default_max_bucket_size")]
    pub max_bucket_size: usize,
    #[serde(default = "default_max_context_commits")]
    pub max_context_commits: usize,
    /// Single-linkage cosine distance threshold for sub-clustering oversized
    /// buckets (members within this distance merge into one sub-cluster).
    #[serde(default = "default_cluster_distance_threshold")]
    pub cluster_distance_threshold: f64,
    #[serde(default = "default_staging_dir")]
    pub staging_dir: PathBuf,
    #[serde(default)]
    pub judge: RouteJudgeConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RouteJudgeConfig {
    pub model_preference: Option<String>,
    #[serde(default = "default_max_total_tokens")]
    pub max_total_tokens: u64,
    #[serde(default = "default_max_dollar_cost")]
    pub max_dollar_cost: f64,
    /// Upper bound on output tokens requested per judge LLM call.
    #[serde(default = "default_judge_max_output_tokens")]
    pub judge_max_output_tokens: u64,
    #[serde(default = "default_verify")]
    pub verify: bool,
}

fn default_min_waste_score() -> u8 {
    4
}
fn default_max_bucket_size() -> usize {
    20
}
fn default_max_context_commits() -> usize {
    6
}
fn default_cluster_distance_threshold() -> f64 {
    0.30
}
fn default_judge_max_output_tokens() -> u64 {
    2048
}
fn default_staging_dir() -> PathBuf {
    PathBuf::from("target/audit/effort-route")
}
fn default_max_total_tokens() -> u64 {
    5_000_000
}
fn default_max_dollar_cost() -> f64 {
    5.00
}
fn default_verify() -> bool {
    true
}

impl Default for RouteJudgeConfig {
    fn default() -> Self {
        Self {
            model_preference: None,
            max_total_tokens: default_max_total_tokens(),
            max_dollar_cost: default_max_dollar_cost(),
            judge_max_output_tokens: default_judge_max_output_tokens(),
            verify: default_verify(),
        }
    }
}

impl Default for EffortRouteConfig {
    fn default() -> Self {
        Self {
            min_waste_score: default_min_waste_score(),
            max_bucket_size: default_max_bucket_size(),
            max_context_commits: default_max_context_commits(),
            cluster_distance_threshold: default_cluster_distance_threshold(),
            staging_dir: default_staging_dir(),
            judge: RouteJudgeConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_spec() {
        let c = EffortRouteConfig::default();
        assert_eq!(c.min_waste_score, 4);
        assert_eq!(c.max_bucket_size, 20);
        assert_eq!(c.max_context_commits, 6);
        assert_eq!(c.cluster_distance_threshold, 0.30);
        assert!(c.judge.verify);
        assert_eq!(c.judge.max_total_tokens, 5_000_000);
        assert_eq!(c.judge.judge_max_output_tokens, 2048);
    }

    #[test]
    fn partial_toml_inherits_defaults() {
        let c: EffortRouteConfig = toml::from_str(
            r#"
            min_waste_score = 6
            [judge]
            verify = false
        "#,
        )
        .unwrap();
        assert_eq!(c.min_waste_score, 6);
        assert!(!c.judge.verify);
        assert_eq!(c.max_bucket_size, 20);
    }
}
