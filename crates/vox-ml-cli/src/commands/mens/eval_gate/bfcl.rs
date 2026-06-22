//! B7.2 — BFCL (Berkeley Function Calling Leaderboard) gate.
//!
//! Gates on function-calling accuracy from `bfcl_results.json` in the run dir.
//! Enforces:
//!   1. Beat-base: trained accuracy > baseline + margin (with CI overlap consideration)
//!   2. Per-rung overrides: different min_accuracy thresholds per adapter rung
//!   3. Regression guard: vs prior registered adapter in `prior_adapter_card.json`

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::baseline::{BaselineEntry, beat_base};
use super::check_run::GateResult;
use super::io::read_utf8_path_capped;

/// BFCL accuracy gate configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BfclGate {
    /// Minimum accuracy required (0.0–1.0). Used when no baseline is available.
    #[serde(default)]
    pub min_accuracy: f64,
    /// If true, a failing gate blocks the run.
    #[serde(default)]
    pub block: bool,
    /// Basename of the metrics file in the run directory.
    #[serde(default = "default_bfcl_metrics_file")]
    pub metrics_file: String,
    /// Maximum tolerated regression from a previously registered adapter.
    #[serde(default = "default_regression_max_drop")]
    pub regression_max_drop: f64,
    /// Per-rung accuracy overrides. Key = the rung label written to the adapter
    /// card's `base_rung` field, which equals the resolved **preset name** (e.g.
    /// "qwen3_dev_cpu", "qwen3_16g", "qwen3_24g"). These are the only identifiers
    /// the resolver actually emits — a key that is not a real preset silently
    /// no-ops (the default threshold applies instead).
    #[serde(default)]
    pub per_rung_overrides: HashMap<String, f64>,
}

impl Default for BfclGate {
    fn default() -> Self {
        Self {
            min_accuracy: 0.0,
            block: false,
            metrics_file: default_bfcl_metrics_file(),
            regression_max_drop: default_regression_max_drop(),
            per_rung_overrides: HashMap::new(),
        }
    }
}

fn default_bfcl_metrics_file() -> String {
    "bfcl_results.json".to_string()
}

fn default_regression_max_drop() -> f64 {
    0.03
}

/// Check the BFCL gate for a given run directory.
///
/// # "Not applicable" semantics
/// When `bfcl_results.json` is absent AND `gate.block == false`, returns a
/// passing `GateResult` with `passed: true` so non-agentic spokes aren't
/// blocked by a gate that doesn't apply.
///
/// # Arguments
/// - `run_dir`: path to the run output directory.
/// - `gate`: BFCL gate config (from eval-gates-bfcl.yaml or policy field).
/// - `baseline_entry`: optional baseline measurement for the beat-base check.
/// - `rung_key`: optional rung identifier for per-rung override lookup.
pub fn check_bfcl(
    run_dir: &Path,
    gate: &BfclGate,
    baseline_entry: Option<&BaselineEntry>,
    rung_key: Option<&str>,
) -> anyhow::Result<GateResult> {
    let metrics_file = {
        let p = std::path::Path::new(&gate.metrics_file);
        let fname = p.file_name().unwrap_or_default();
        if fname.is_empty() {
            run_dir.join(default_bfcl_metrics_file())
        } else {
            run_dir.join(fname)
        }
    };

    // "Not applicable" branch — metrics file absent and gate is non-blocking
    if !metrics_file.exists() {
        if !gate.block {
            return Ok(GateResult {
                name: "bfcl_accuracy".to_string(),
                passed: true,
                message: format!(
                    "bfcl_results not found at {} — not applicable (block=false)",
                    metrics_file.display()
                ),
                block: false,
            });
        } else {
            return Ok(GateResult {
                name: "bfcl_accuracy".to_string(),
                passed: false,
                message: format!(
                    "bfcl_results.json missing: {} — run BFCL eval first",
                    metrics_file.display()
                ),
                block: true,
            });
        }
    }

    // Read metrics
    let content = read_utf8_path_capped(&metrics_file)?;
    let v: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| anyhow::anyhow!("{}: invalid JSON ({})", metrics_file.display(), e))?;

    let trained_accuracy = v.get("accuracy").and_then(|x| x.as_f64()).unwrap_or(0.0);

    // Effective minimum: per-rung override takes precedence over default
    let effective_min = rung_key
        .and_then(|rk| gate.per_rung_overrides.get(rk))
        .copied()
        .unwrap_or(gate.min_accuracy);

    // Beat-base check (if baseline provided)
    let (beat_ok, beat_msg) = if let Some(bl) = baseline_entry {
        let ok = beat_base(trained_accuracy, bl, 0.0);
        let msg = format!(
            "beat_base={} (trained={:.4}, baseline={:.4}, ci=[{:.4},{:.4}])",
            ok, trained_accuracy, bl.value, bl.ci_low, bl.ci_high
        );
        (ok, msg)
    } else {
        (true, "no baseline provided — beat-base skipped".to_string())
    };

    // Absolute threshold check
    let threshold_ok = trained_accuracy >= effective_min;

    // Regression guard: compare vs prior adapter card
    let prior_adapter_path = run_dir.join("prior_adapter_card.json");
    let (regression_ok, regression_msg) = if prior_adapter_path.exists() {
        let prior_content = read_utf8_path_capped(&prior_adapter_path)?;
        match serde_json::from_str::<serde_json::Value>(&prior_content) {
            Ok(prior) => {
                let prior_acc = prior
                    .get("metrics")
                    .and_then(|m| m.get("bfcl_accuracy"))
                    .and_then(|x| x.as_f64())
                    .or_else(|| prior.get("bfcl_accuracy").and_then(|x| x.as_f64()))
                    .unwrap_or(0.0);
                let drop = prior_acc - trained_accuracy;
                let ok = drop <= gate.regression_max_drop;
                (
                    ok,
                    format!(
                        "regression_guard: prior={:.4} current={:.4} drop={:.4} max_drop={:.4}",
                        prior_acc, trained_accuracy, drop, gate.regression_max_drop
                    ),
                )
            }
            Err(_) => (
                true,
                "prior_adapter_card.json unparseable — skipped".to_string(),
            ),
        }
    } else {
        (
            true,
            "no prior adapter — regression check skipped".to_string(),
        )
    };

    let passed = beat_ok && threshold_ok && regression_ok;
    let message = format!(
        "accuracy={:.4} (min={:.4}) rung={} | {} | {}",
        trained_accuracy,
        effective_min,
        rung_key.unwrap_or("default"),
        beat_msg,
        regression_msg,
    );

    Ok(GateResult {
        name: "bfcl_accuracy".to_string(),
        passed,
        message,
        block: gate.block,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::mens::eval_gate::baseline::BaselineEntry;

    fn gate(block: bool) -> BfclGate {
        BfclGate {
            min_accuracy: 0.0,
            block,
            metrics_file: "bfcl_results.json".to_string(),
            regression_max_drop: 0.03,
            per_rung_overrides: HashMap::new(),
        }
    }

    fn baseline(value: f64, ci_low: f64, ci_high: f64) -> BaselineEntry {
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
    fn not_applicable_when_absent_and_block_false() {
        let dir = tempfile::tempdir().unwrap();
        let result = check_bfcl(dir.path(), &gate(false), None, None).unwrap();
        assert!(
            result.passed,
            "should pass as not applicable: {}",
            result.message
        );
        assert!(!result.block);
        assert!(result.message.contains("not applicable"));
    }

    #[test]
    fn fails_with_block_true_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        let result = check_bfcl(dir.path(), &gate(true), None, None).unwrap();
        assert!(
            !result.passed,
            "should fail when block=true and file absent"
        );
        assert!(result.block);
    }

    #[test]
    fn passes_when_above_baseline() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("bfcl_results.json"),
            r#"{"accuracy": 0.75, "total": 100}"#,
        )
        .unwrap();
        let bl = baseline(0.60, 0.55, 0.65);
        let result = check_bfcl(dir.path(), &gate(true), Some(&bl), None).unwrap();
        assert!(
            result.passed,
            "0.75 should beat baseline 0.60: {}",
            result.message
        );
    }

    #[test]
    fn blocks_below_baseline() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("bfcl_results.json"),
            r#"{"accuracy": 0.50, "total": 100}"#,
        )
        .unwrap();
        let bl = baseline(0.60, 0.55, 0.65);
        let result = check_bfcl(dir.path(), &gate(true), Some(&bl), None).unwrap();
        assert!(!result.passed, "0.50 should not beat baseline 0.60");
        assert!(result.block);
    }

    #[test]
    fn per_rung_override_threshold() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("bfcl_results.json"),
            r#"{"accuracy": 0.55}"#,
        )
        .unwrap();
        let mut g = gate(true);
        g.min_accuracy = 0.70; // default high
        // DEFAULTS F1: use a REAL rung label (the CPU/dev preset name actually emitted
        // into base_rung), not the old fictional "qwen3_0_6b_cpu" that no-op'd.
        g.per_rung_overrides
            .insert("qwen3_dev_cpu".to_string(), 0.50); // lower override for CPU
        let result = check_bfcl(dir.path(), &g, None, Some("qwen3_dev_cpu")).unwrap();
        assert!(
            result.passed,
            "0.55 >= per_rung override 0.50 should pass: {}",
            result.message
        );
    }

    #[test]
    fn per_rung_override_keys_are_real_preset_rungs() {
        // DEFAULTS F1 guard: every per_rung_override key must be a real rung label,
        // i.e. a known preset name emitted into the adapter card's base_rung field.
        // A fictional key (the old "qwen3_8b_4080"/"qwen3_0_6b_cpu") silently no-ops.
        use vox_populi::mens::KNOWN_PRESETS;
        for rung in [
            "qwen3_dev_cpu",
            "qwen3_16g",
            "qwen3_24g",
            "qwen3_48g",
            "qwen3_96g",
        ] {
            assert!(
                KNOWN_PRESETS.contains(&rung),
                "override rung '{rung}' must be a real KNOWN_PRESETS id"
            );
        }

        // End-to-end: an override keyed on a real rung is actually applied.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("bfcl_results.json"),
            r#"{"accuracy": 0.55}"#,
        )
        .unwrap();
        let mut g = gate(true);
        g.min_accuracy = 0.90; // default would fail
        g.per_rung_overrides.insert("qwen3_16g".to_string(), 0.50);
        let applied = check_bfcl(dir.path(), &g, None, Some("qwen3_16g")).unwrap();
        assert!(
            applied.passed,
            "real-rung override (qwen3_16g→0.50) must apply, lowering the gate: {}",
            applied.message
        );
        // And a fictional key must NOT apply (default 0.90 still in force → fail).
        let mut g2 = gate(true);
        g2.min_accuracy = 0.90;
        g2.per_rung_overrides
            .insert("qwen3_8b_4080".to_string(), 0.50); // fictional
        let not_applied = check_bfcl(dir.path(), &g2, None, Some("qwen3_16g")).unwrap();
        assert!(
            !not_applied.passed,
            "fictional override key must not apply; default 0.90 should still fail 0.55"
        );
    }

    #[test]
    fn per_rung_falls_back_to_default_when_key_absent() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("bfcl_results.json"),
            r#"{"accuracy": 0.55}"#,
        )
        .unwrap();
        let mut g = gate(true);
        g.min_accuracy = 0.70; // default too high
        g.per_rung_overrides.insert("other_rung".to_string(), 0.40);
        let result = check_bfcl(dir.path(), &g, None, Some("missing_rung")).unwrap();
        assert!(
            !result.passed,
            "0.55 < default 0.70 should fail when rung key absent: {}",
            result.message
        );
    }

    #[test]
    fn regression_guard_fires_when_drop_exceeds_threshold() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("bfcl_results.json"),
            r#"{"accuracy": 0.55}"#,
        )
        .unwrap();
        // Prior adapter had 0.65; drop = 0.10 > max_drop 0.03
        std::fs::write(
            dir.path().join("prior_adapter_card.json"),
            r#"{"metrics": {"bfcl_accuracy": 0.65}}"#,
        )
        .unwrap();
        let result = check_bfcl(dir.path(), &gate(true), None, None).unwrap();
        assert!(!result.passed, "regression drop 0.10 > 0.03 should fail");
        assert!(result.message.contains("regression_guard"));
    }

    #[test]
    fn regression_guard_passes_within_tolerance() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("bfcl_results.json"),
            r#"{"accuracy": 0.62}"#,
        )
        .unwrap();
        // Prior 0.63; drop = 0.01 <= max_drop 0.03
        std::fs::write(
            dir.path().join("prior_adapter_card.json"),
            r#"{"metrics": {"bfcl_accuracy": 0.63}}"#,
        )
        .unwrap();
        let result = check_bfcl(dir.path(), &gate(true), None, None).unwrap();
        assert!(
            result.passed,
            "regression drop 0.01 <= 0.03 should pass: {}",
            result.message
        );
    }
}
