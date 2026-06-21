//! B8.4 — Parity report: per-spoke training result vs baseline + Flash/Sonnet gap.
//!
//! After a training run completes and `vox mens eval-gate` has measured the
//! trained adapter's metric, call `compute_parity_entry` to compare the result
//! against the captured pre-training baseline and optional reference model
//! metrics.  Aggregate entries into a `ParityReport` and persist it so the
//! north-star gap (Flash / Sonnet parity) is permanently recorded even when
//! the individual run directories are cleaned up.
//!
//! # V1 acceptance rule
//!
//! `v1_accepted = true` iff *every* entry's `beats_base` is `true`.
//! Flash/Sonnet parity gaps are informational — they do **not** gate V1.

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::Path;

use super::eval_gate::baseline::{BaselineEntry, beat_base};

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// A single parity measurement for one training spoke.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParityEntry {
    /// Spoke identifier (e.g. `"vox-lang"`, `"rust"`).
    pub spoke: String,
    /// The primary metric value produced by the trained adapter (e.g. BFCL accuracy).
    pub metric: f64,
    /// The baseline metric (point estimate) from the pre-training baseline capture.
    pub baseline_metric: f64,
    /// Whether the trained adapter beats its base rung (`metric > baseline_metric`
    /// using the CI-aware `beat_base` function with margin 0.0).
    pub beats_base: bool,
    /// Reference metric for Gemini Flash (informational north-star; `None` if not
    /// yet measured).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flash_reference: Option<f64>,
    /// Reference metric for Claude Sonnet (informational north-star; `None` if not
    /// yet measured).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sonnet_reference: Option<f64>,
    /// `metric - flash_reference` (positive = beats Flash, negative = below Flash).
    /// `None` when `flash_reference` is absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap_to_flash: Option<f64>,
    /// `metric - sonnet_reference` (positive = beats Sonnet, negative = below Sonnet).
    /// `None` when `sonnet_reference` is absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap_to_sonnet: Option<f64>,
}

/// A collection of parity entries for a completed training session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParityReport {
    pub entries: Vec<ParityEntry>,
    /// `true` iff every entry's `beats_base` is `true`.
    /// This is the V1 acceptance gate (Flash/Sonnet parity is informational only).
    pub v1_accepted: bool,
    /// ISO-8601 creation timestamp.
    pub created: String,
}

// ---------------------------------------------------------------------------
// Constructors / helpers
// ---------------------------------------------------------------------------

/// Build a `ParityEntry` by comparing a trained metric against the pre-training
/// baseline and optional reference model scores.
///
/// * `spoke` — spoke identifier.
/// * `trained_metric` — the primary metric from the trained adapter (e.g.
///   `bfcl_accuracy`).
/// * `baseline_entry` — the captured `BaselineEntry` for this spoke (must
///   come from `eval_gate::baseline`).
/// * `flash_ref` — optional Flash reference metric for the same spoke/task.
/// * `sonnet_ref` — optional Sonnet reference metric.
///
/// `beats_base` is computed with `beat_base(trained_metric, baseline_entry, 0.0)`,
/// which is CI-aware (see `eval_gate::baseline::beat_base`).
pub fn compute_parity_entry(
    spoke: &str,
    trained_metric: f64,
    baseline_entry: &BaselineEntry,
    flash_ref: Option<f64>,
    sonnet_ref: Option<f64>,
) -> ParityEntry {
    let beats = beat_base(trained_metric, baseline_entry, 0.0);
    ParityEntry {
        spoke: spoke.to_string(),
        metric: trained_metric,
        baseline_metric: baseline_entry.value,
        beats_base: beats,
        flash_reference: flash_ref,
        sonnet_reference: sonnet_ref,
        gap_to_flash: flash_ref.map(|r| trained_metric - r),
        gap_to_sonnet: sonnet_ref.map(|r| trained_metric - r),
    }
}

/// Construct a `ParityReport` from a list of entries.
/// `v1_accepted` is derived automatically from the entries.
pub fn build_parity_report(entries: Vec<ParityEntry>) -> ParityReport {
    let v1_accepted = !entries.is_empty() && entries.iter().all(|e| e.beats_base);
    ParityReport {
        entries,
        v1_accepted,
        created: Utc::now().to_rfc3339(),
    }
}

// ---------------------------------------------------------------------------
// Persistence
// ---------------------------------------------------------------------------

/// Write a `ParityReport` as pretty-printed JSON to `path`.
pub fn write_parity_report(path: &Path, report: &ParityReport) -> Result<()> {
    let json = serde_json::to_string_pretty(report).context("serialising ParityReport to JSON")?;
    std::fs::write(path, json)
        .with_context(|| format!("writing parity report to {}", path.display()))?;
    Ok(())
}

/// Load a `ParityReport` from a JSON file at `path`.
pub fn load_parity_report(path: &Path) -> Result<ParityReport> {
    let content = vox_bounded_fs::read_utf8_path_capped(path)
        .with_context(|| format!("reading parity report at {}", path.display()))?;
    let report: ParityReport = serde_json::from_str(&content)
        .with_context(|| format!("parsing parity report at {}", path.display()))?;
    Ok(report)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::mens::eval_gate::baseline::BaselineEntry;

    fn baseline(spoke: &str, value: f64, ci_low: f64, ci_high: f64) -> BaselineEntry {
        BaselineEntry {
            spoke: spoke.to_string(),
            metric_name: "bfcl_accuracy".to_string(),
            value,
            sample_size: 100,
            ci_low,
            ci_high,
            pass_at_k: 1,
            judge_identity: None,
        }
    }

    // -----------------------------------------------------------------------
    // beats_base logic
    // -----------------------------------------------------------------------

    #[test]
    fn beats_base_when_clearly_above_ci_high() {
        let b = baseline("vox-lang", 0.60, 0.55, 0.65);
        let entry = compute_parity_entry("vox-lang", 0.72, &b, None, None);
        assert!(entry.beats_base, "0.72 should beat ci_high=0.65");
    }

    #[test]
    fn does_not_beat_base_when_below_value() {
        let b = baseline("rust", 0.80, 0.75, 0.85);
        let entry = compute_parity_entry("rust", 0.78, &b, None, None);
        assert!(!entry.beats_base, "0.78 < value 0.80 should not beat base");
    }

    #[test]
    fn does_not_beat_base_equal_to_value() {
        let b = baseline("rust", 0.80, 0.75, 0.85);
        // beat_base with margin=0.0 requires strictly greater than value
        let entry = compute_parity_entry("rust", 0.80, &b, None, None);
        assert!(
            !entry.beats_base,
            "exactly equal to value should not beat base"
        );
    }

    // -----------------------------------------------------------------------
    // Gap arithmetic
    // -----------------------------------------------------------------------

    #[test]
    fn gap_to_flash_computed_correctly() {
        let b = baseline("vox-lang", 0.60, 0.55, 0.65);
        let entry = compute_parity_entry("vox-lang", 0.65, &b, Some(0.70), None);
        let gap = entry.gap_to_flash.expect("gap should be set");
        // 0.65 - 0.70 = -0.05
        assert!((gap - (-0.05)).abs() < 1e-9, "gap_to_flash = {gap}");
    }

    #[test]
    fn gap_to_sonnet_computed_correctly() {
        let b = baseline("vox-lang", 0.60, 0.55, 0.65);
        let entry = compute_parity_entry("vox-lang", 0.75, &b, None, Some(0.72));
        let gap = entry.gap_to_sonnet.expect("gap should be set");
        // 0.75 - 0.72 = 0.03
        assert!((gap - 0.03).abs() < 1e-9, "gap_to_sonnet = {gap}");
    }

    #[test]
    fn gaps_are_none_when_references_absent() {
        let b = baseline("vox-lang", 0.60, 0.55, 0.65);
        let entry = compute_parity_entry("vox-lang", 0.65, &b, None, None);
        assert!(entry.gap_to_flash.is_none());
        assert!(entry.gap_to_sonnet.is_none());
    }

    // -----------------------------------------------------------------------
    // v1_accepted
    // -----------------------------------------------------------------------

    #[test]
    fn v1_accepted_when_all_entries_beat_base() {
        let b1 = baseline("vox-lang", 0.60, 0.55, 0.65);
        let b2 = baseline("rust", 0.80, 0.75, 0.85);
        let entries = vec![
            compute_parity_entry("vox-lang", 0.70, &b1, None, None),
            compute_parity_entry("rust", 0.85, &b2, None, None),
        ];
        let report = build_parity_report(entries);
        assert!(report.v1_accepted, "all entries beat base → v1_accepted");
    }

    #[test]
    fn v1_not_accepted_when_any_entry_fails_to_beat_base() {
        let b1 = baseline("vox-lang", 0.60, 0.55, 0.65);
        let b2 = baseline("rust", 0.80, 0.75, 0.85);
        let entries = vec![
            compute_parity_entry("vox-lang", 0.70, &b1, None, None), // beats
            compute_parity_entry("rust", 0.79, &b2, None, None),     // does NOT beat
        ];
        let report = build_parity_report(entries);
        assert!(!report.v1_accepted, "one entry fails → v1_not_accepted");
    }

    #[test]
    fn v1_not_accepted_when_entries_empty() {
        let report = build_parity_report(vec![]);
        assert!(!report.v1_accepted, "empty entries → not accepted");
    }

    // -----------------------------------------------------------------------
    // Round-trip persistence
    // -----------------------------------------------------------------------

    #[test]
    fn round_trip_write_and_load() {
        let b1 = baseline("vox-lang", 0.60, 0.55, 0.65);
        let b2 = baseline("rust", 0.80, 0.75, 0.85);
        let entries = vec![
            compute_parity_entry("vox-lang", 0.70, &b1, Some(0.75), Some(0.80)),
            compute_parity_entry("rust", 0.85, &b2, None, Some(0.90)),
        ];
        let report = build_parity_report(entries);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("parity.json");

        write_parity_report(&path, &report).expect("write should succeed");
        let loaded = load_parity_report(&path).expect("load should succeed");

        assert_eq!(loaded.entries.len(), 2);
        assert_eq!(loaded.v1_accepted, report.v1_accepted);
        assert_eq!(loaded.entries[0].spoke, "vox-lang");
        // Use approximate comparison to tolerate f64 round-trip through JSON.
        let gap_flash = loaded.entries[0]
            .gap_to_flash
            .expect("gap_to_flash should be present");
        assert!(
            (gap_flash - (0.70 - 0.75)).abs() < 1e-9,
            "gap_to_flash={gap_flash}"
        );
        assert_eq!(loaded.entries[1].spoke, "rust");
        assert!(loaded.entries[1].gap_to_flash.is_none());
        let gap_sonnet = loaded.entries[1]
            .gap_to_sonnet
            .expect("gap_to_sonnet should be present");
        assert!(
            (gap_sonnet - (0.85 - 0.90)).abs() < 1e-9,
            "gap_to_sonnet={gap_sonnet}"
        );
        // created timestamp round-trips
        assert!(!loaded.created.is_empty());
    }

    #[test]
    fn load_rejects_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.json");
        std::fs::write(&path, "{ not valid json").unwrap();
        assert!(load_parity_report(&path).is_err());
    }

    // -----------------------------------------------------------------------
    // Serialisation: optional fields are omitted when None
    // -----------------------------------------------------------------------

    #[test]
    fn optional_fields_omitted_in_json_when_none() {
        let b = baseline("vox-lang", 0.60, 0.55, 0.65);
        let entry = compute_parity_entry("vox-lang", 0.65, &b, None, None);
        let report = build_parity_report(vec![entry]);
        let json = serde_json::to_string(&report).unwrap();
        // None fields should not appear in the serialised output
        assert!(
            !json.contains("flash_reference"),
            "flash_reference should be omitted"
        );
        assert!(
            !json.contains("sonnet_reference"),
            "sonnet_reference should be omitted"
        );
        assert!(
            !json.contains("gap_to_flash"),
            "gap_to_flash should be omitted"
        );
        assert!(
            !json.contains("gap_to_sonnet"),
            "gap_to_sonnet should be omitted"
        );
    }
}
