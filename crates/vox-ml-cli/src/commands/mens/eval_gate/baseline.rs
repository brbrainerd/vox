//! B7.1 — Baseline capture: structs and helpers for recording and comparing
//! per-spoke metric baselines.
//!
//! A baseline must be captured on the *base* model (before any adapter is
//! applied) so that `beat_base` can assert the trained adapter improves on it.

use anyhow::{Context, Result};
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

/// Wilson score interval for a binomial proportion.
///
/// Returns `(ci_low, ci_high)` clamped to `[0, 1]`. `z` is the standard-normal
/// critical value (use `1.96` for a 95% interval). For `n == 0` the interval is
/// degenerate — there is no sample to bound — so `(p, p)` is returned.
pub fn wilson_ci(p: f64, n: usize, z: f64) -> (f64, f64) {
    if n == 0 {
        return (p.clamp(0.0, 1.0), p.clamp(0.0, 1.0));
    }
    let n = n as f64;
    let z2 = z * z;
    let denom = 1.0 + z2 / n;
    let center = (p + z2 / (2.0 * n)) / denom;
    let margin = (z / denom) * ((p * (1.0 - p) / n) + z2 / (4.0 * n * n)).sqrt();
    let low = (center - margin).clamp(0.0, 1.0);
    let high = (center + margin).clamp(0.0, 1.0);
    (low, high)
}

/// Read a base-model eval result file (`bfcl_results.json` with shape
/// `{"accuracy": f64, "total": usize}`) from `base_eval_dir` and build a
/// [`BaselineReport`] containing a single `bfcl_accuracy` entry.
///
/// The entry's confidence interval is a Wilson 95% score interval computed from
/// `(accuracy, total)`. `created` is caller-supplied (ISO-8601) so the function
/// is deterministic and testable.
///
/// Fail-closed: errors if `bfcl_results.json` is absent. A baseline MUST be
/// captured from a real base eval — never fabricated — so the beat-base gate has
/// a genuine point of comparison.
pub fn capture_baseline(
    base_eval_dir: &Path,
    spoke: &str,
    created: &str,
    judge_identity: Option<String>,
) -> Result<BaselineReport> {
    let metrics_path = base_eval_dir.join("bfcl_results.json");
    if !metrics_path.exists() {
        anyhow::bail!(
            "base eval file missing: {} — run the BFCL eval harness against the BASE model \
             (no adapter) first; a baseline must be captured from a real eval, not fabricated",
            metrics_path.display()
        );
    }
    let content = vox_bounded_fs::read_utf8_path_capped(&metrics_path)
        .with_context(|| format!("read {}", metrics_path.display()))?;
    let v: serde_json::Value = serde_json::from_str(&content)
        .with_context(|| format!("{}: invalid JSON", metrics_path.display()))?;

    let accuracy = v.get("accuracy").and_then(|x| x.as_f64()).ok_or_else(|| {
        anyhow::anyhow!(
            "{}: missing required `accuracy` field",
            metrics_path.display()
        )
    })?;
    let total = v.get("total").and_then(|x| x.as_u64()).unwrap_or(0) as usize;

    let (ci_low, ci_high) = wilson_ci(accuracy, total, 1.96);

    let entry = BaselineEntry {
        spoke: spoke.to_string(),
        metric_name: "bfcl_accuracy".to_string(),
        value: accuracy,
        sample_size: total,
        ci_low,
        ci_high,
        pass_at_k: 1,
        judge_identity,
    };

    Ok(BaselineReport {
        entries: vec![entry],
        created: created.to_string(),
    })
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

    // -----------------------------------------------------------------------
    // wilson_ci
    // -----------------------------------------------------------------------

    #[test]
    fn wilson_ci_brackets_point_estimate() {
        let (lo, hi) = wilson_ci(0.6, 100, 1.96);
        assert!(lo < 0.6, "ci_low {lo} should be below p=0.6");
        assert!(hi > 0.6, "ci_high {hi} should be above p=0.6");
        assert!((0.0..=1.0).contains(&lo), "ci_low {lo} in [0,1]");
        assert!((0.0..=1.0).contains(&hi), "ci_high {hi} in [0,1]");
    }

    #[test]
    fn wilson_ci_degenerate_when_n_zero() {
        let (lo, hi) = wilson_ci(0.42, 0, 1.96);
        assert_eq!(lo, 0.42);
        assert_eq!(hi, 0.42);
    }

    #[test]
    fn wilson_ci_clamps_upper_bound_at_one() {
        let (lo, hi) = wilson_ci(1.0, 50, 1.96);
        assert_eq!(hi, 1.0, "ci_high must clamp to 1.0 at p=1.0");
        assert!(lo < 1.0, "ci_low {lo} should still be below 1.0");
        assert!(lo >= 0.0, "ci_low {lo} >= 0");
    }

    // -----------------------------------------------------------------------
    // capture_baseline
    // -----------------------------------------------------------------------

    #[test]
    fn capture_baseline_reads_fixture_and_builds_entry() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("bfcl_results.json"),
            r#"{"accuracy": 0.6, "total": 100}"#,
        )
        .unwrap();
        let report = capture_baseline(
            dir.path(),
            "tool-selection",
            "2026-06-21T00:00:00Z",
            Some("qwen3-4b-base".to_string()),
        )
        .unwrap();
        assert_eq!(report.created, "2026-06-21T00:00:00Z");
        assert_eq!(report.entries.len(), 1);
        let e = &report.entries[0];
        assert_eq!(e.spoke, "tool-selection");
        assert_eq!(e.metric_name, "bfcl_accuracy");
        assert_eq!(e.value, 0.6);
        assert_eq!(e.sample_size, 100);
        assert_eq!(e.pass_at_k, 1);
        assert_eq!(e.judge_identity, Some("qwen3-4b-base".to_string()));
        // Non-degenerate CI brackets the point estimate.
        assert!(e.ci_low < 0.6 && e.ci_high > 0.6, "CI should bracket value");
    }

    #[test]
    fn capture_baseline_errors_when_file_absent() {
        let dir = tempfile::tempdir().unwrap();
        // No bfcl_results.json written — fail-closed.
        let err = capture_baseline(dir.path(), "vox-lang", "2026-06-21T00:00:00Z", None)
            .expect_err("must fail when base eval file is absent");
        assert!(
            err.to_string().contains("bfcl_results.json"),
            "error should name the missing file: {err}"
        );
    }

    // -----------------------------------------------------------------------
    // End-to-end producer → consumer loop. The producer (capture_baseline +
    // save_baseline) writes baseline_report.json that the consumer (check_run →
    // check_bfcl → beat_base) reads. Proves the beat-base gate is live.
    // -----------------------------------------------------------------------

    fn run_loop(base_accuracy: f64, trained_accuracy: f64) -> bool {
        use crate::commands::mens::eval_gate::check_run::check_run;

        let train_dir = tempfile::tempdir().unwrap();
        let base_dir = tempfile::tempdir().unwrap();

        // 1. Base eval → capture_baseline → save into the TRAIN run dir.
        std::fs::write(
            base_dir.path().join("bfcl_results.json"),
            format!(r#"{{"accuracy": {base_accuracy}, "total": 100}}"#),
        )
        .unwrap();
        let report =
            capture_baseline(base_dir.path(), "vox-lang", "2026-06-21T00:00:00Z", None).unwrap();
        save_baseline(&train_dir.path().join("baseline_report.json"), &report).unwrap();

        // 2. Trained adapter eval + manifest in the train run dir.
        std::fs::write(
            train_dir.path().join("bfcl_results.json"),
            format!(r#"{{"accuracy": {trained_accuracy}, "total": 100}}"#),
        )
        .unwrap();
        std::fs::write(
            train_dir.path().join("training_manifest.json"),
            r#"{"spoke":"vox-lang"}"#,
        )
        .unwrap();

        // 3. Policy with bfcl_accuracy.block=true, threshold 0 (only beat-base gates).
        let policy_path = train_dir.path().join("policy.yaml");
        std::fs::write(
            &policy_path,
            "version: \"1\"\nbfcl_accuracy:\n  min_accuracy: 0.0\n  block: true\n",
        )
        .unwrap();

        let results = check_run(train_dir.path(), &policy_path).expect("check_run");
        let gate = results
            .iter()
            .find(|r| r.name == "bfcl_accuracy")
            .expect("bfcl_accuracy gate present");
        assert!(
            gate.message.contains("beat_base"),
            "beat-base comparison must run through the produced baseline: {}",
            gate.message
        );
        gate.passed
    }

    #[test]
    fn produced_baseline_gates_pass_when_trained_beats_base() {
        // 0.75 trained beats 0.60 base → gate passes.
        assert!(
            run_loop(0.60, 0.75),
            "trained 0.75 should beat produced baseline 0.60"
        );
    }

    #[test]
    fn produced_baseline_gates_fail_when_trained_below_base() {
        // 0.50 trained does not beat 0.60 base → gate fails. Proves the
        // producer→consumer loop actually gates (not silently skipping).
        assert!(
            !run_loop(0.60, 0.50),
            "trained 0.50 must NOT beat produced baseline 0.60"
        );
    }
}
