//! B7.1 — Baseline capture: structs and helpers for recording and comparing
//! per-spoke metric baselines.
//!
//! A baseline must be captured on the *base* model (before any adapter is
//! applied) so that `beat_base` can assert the trained adapter improves on it.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A single baseline measurement for one spoke / metric combination.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BaselineEntry {
    /// Spoke identifier (e.g. "vox-lang", "rust", "tool-selection").
    pub spoke: String,
    /// Metric name (e.g. "bfcl_accuracy", "pass_at_k", "vox_parse_rate").
    pub metric_name: String,
    /// Point estimate.
    pub value: f64,
    /// Number of samples used to compute this estimate.
    pub sample_size: usize,
    /// Lower bound of 95% confidence interval.
    pub ci_low: f64,
    /// Upper bound of 95% confidence interval.
    pub ci_high: f64,
    /// k value if this is a pass@k metric.
    pub pass_at_k: u32,
    /// Identity of the judge / evaluator that produced this entry (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub judge_identity: Option<String>,
}

/// A collection of baseline entries captured at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaselineReport {
    pub entries: Vec<BaselineEntry>,
    /// ISO-8601 timestamp when this baseline was created.
    pub created: String,
}

/// Load a `BaselineReport` from a JSON file.
pub fn load_baseline(path: &Path) -> Result<BaselineReport> {
    let content = vox_bounded_fs::read_utf8_path_capped(path)?;
    let report: BaselineReport = serde_json::from_str(&content)?;
    Ok(report)
}

/// Persist a `BaselineReport` to a JSON file.
pub fn save_baseline(path: &Path, report: &BaselineReport) -> Result<()> {
    let json = serde_json::to_string_pretty(report)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Returns `true` if `candidate` beats `baseline` by at least `margin`,
/// after accounting for CI overlap.
///
/// Algorithm:
///   - If `candidate > baseline.ci_high + margin` → clear beat (returns true).
///   - If `candidate > baseline.value + margin` AND `candidate >= baseline.ci_low`
///     → beat considering CI overlap (returns true).
///   - Otherwise false.
///
/// `margin` is typically a small improvement requirement (e.g. 0.01 = 1 pp).
/// Set `margin = 0.0` to require only marginal improvement.
pub fn beat_base(candidate: f64, baseline: &BaselineEntry, margin: f64) -> bool {
    // Clear beat: above the upper CI bound plus margin
    if candidate > baseline.ci_high + margin {
        return true;
    }
    // Soft beat: above point estimate + margin, and not below CI lower bound
    if candidate > baseline.value + margin && candidate >= baseline.ci_low {
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(value: f64, ci_low: f64, ci_high: f64) -> BaselineEntry {
        BaselineEntry {
            spoke: "vox-lang".to_string(),
            metric_name: "bfcl_accuracy".to_string(),
            value,
            sample_size: 100,
            ci_low,
            ci_high,
            pass_at_k: 1,
            judge_identity: None,
        }
    }

    #[test]
    fn beat_base_clear_win() {
        let b = entry(0.60, 0.55, 0.65);
        // Candidate 0.70 > ci_high (0.65) + margin (0.01) = 0.66 → clear beat
        assert!(beat_base(0.70, &b, 0.01));
    }

    #[test]
    fn beat_base_soft_win_within_ci() {
        let b = entry(0.60, 0.55, 0.70);
        // Candidate 0.65 > value (0.60) + margin (0.01) = 0.61, and >= ci_low (0.55)
        // but 0.65 <= ci_high (0.70) → soft beat
        assert!(beat_base(0.65, &b, 0.01));
    }

    #[test]
    fn beat_base_fails_below_value_plus_margin() {
        let b = entry(0.60, 0.55, 0.65);
        // 0.605 > 0.60 but not > 0.60 + 0.01 = 0.61
        assert!(!beat_base(0.605, &b, 0.01));
    }

    #[test]
    fn beat_base_fails_when_candidate_below_baseline() {
        let b = entry(0.60, 0.55, 0.65);
        assert!(!beat_base(0.50, &b, 0.0));
    }

    #[test]
    fn beat_base_zero_margin_requires_any_improvement() {
        let b = entry(0.60, 0.58, 0.62);
        // 0.601 > 0.60 + 0.0 = 0.60 and >= ci_low → passes with zero margin
        assert!(beat_base(0.601, &b, 0.0));
        // 0.60 is NOT > 0.60, even at zero margin
        assert!(!beat_base(0.60, &b, 0.0));
    }

    #[test]
    fn round_trip_serialize_deserialize() {
        let report = BaselineReport {
            entries: vec![
                entry(0.65, 0.60, 0.70),
                BaselineEntry {
                    spoke: "rust".to_string(),
                    metric_name: "rust_compile_rate".to_string(),
                    value: 0.88,
                    sample_size: 50,
                    ci_low: 0.82,
                    ci_high: 0.94,
                    pass_at_k: 1,
                    judge_identity: Some("qwen3-4b-base".to_string()),
                },
            ],
            created: "2026-06-21T00:00:00Z".to_string(),
        };

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("baseline.json");
        save_baseline(&path, &report).unwrap();
        let loaded = load_baseline(&path).unwrap();
        assert_eq!(loaded.entries.len(), 2);
        assert_eq!(loaded.entries[0], report.entries[0]);
        assert_eq!(loaded.entries[1].spoke, "rust");
        assert_eq!(
            loaded.entries[1].judge_identity,
            Some("qwen3-4b-base".to_string())
        );
        assert_eq!(loaded.created, "2026-06-21T00:00:00Z");
    }

    #[test]
    fn load_baseline_rejects_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.json");
        std::fs::write(&path, "not json").unwrap();
        assert!(load_baseline(&path).is_err());
    }
}
