//! Machine-readable publication-worthiness policy (`contracts/scientia/*.yaml`) and evaluation.

use std::path::Path;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

/// Default contract path relative to repository root.
pub const DEFAULT_CONTRACT_REL_PATH: &str =
    "contracts/scientia/publication-worthiness.default.yaml";

/// JSON Schema path relative to repository root (validated by `vox ci scientia-worthiness-contract`).
pub const CONTRACT_SCHEMA_REL_PATH: &str = "contracts/scientia/publication-worthiness.schema.json";

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct PublicationWorthinessContract {
    pub version: u32,
    pub decision_labels: DecisionLabels,
    pub hard_red_lines: Vec<HardRedLine>,
    pub thresholds: Thresholds,
    pub weights: Weights,
    /// Advisory venue notes only; `evaluate_worthiness` does not execute these checks yet.
    pub venue_profiles: std::collections::BTreeMap<String, VenueProfile>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct DecisionLabels {
    pub publish: String,
    pub ask_for_evidence: String,
    #[serde(rename = "abstain_do_not_publish")]
    pub abstain_do_not_publish: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct HardRedLine {
    pub id: String,
    pub description: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Thresholds {
    pub claim_evidence_coverage_min: f64,
    pub artifact_replayability_min: f64,
    pub before_after_pair_integrity_min: f64,
    pub metadata_completeness_min: f64,
    pub ai_disclosure_compliance_exact: f64,
    pub publish_score_min: f64,
    pub abstain_score_max: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Weights {
    pub epistemic: f64,
    pub reproducibility: f64,
    pub novelty: f64,
    pub reliability: f64,
    pub metadata_policy: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct VenueProfile {
    pub description: String,
    pub required_checks: Vec<String>,
}

/// Inputs for [`evaluate_worthiness`]; typically deserialized from JSON.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct WorthinessInputs {
    /// Red-line ids that the caller attests were violated (must match enabled contract rows).
    #[serde(default)]
    pub red_line_violation_ids: Vec<String>,
    #[serde(default)]
    pub repeated_unresolved_contradiction: bool,
    pub claim_evidence_coverage: f64,
    /// Operator-declared replayability. When `artifact_replayability_measured`
    /// is `Some`, that measured value supersedes this declared one for
    /// hard-gate checks. Producers of this struct should keep both populated
    /// so consumers can compare declared vs measured during diagnostics.
    pub artifact_replayability: f64,
    /// Phase B: measured replayability written back by `vox-replay-runner`
    /// after sandboxed re-execution of the manifest's RO-Crate `mainEntity`.
    /// `None` means "replay has not been measured yet"; downstream gates
    /// fall back to `artifact_replayability` (declared).
    #[serde(default)]
    pub artifact_replayability_measured: Option<f64>,
    pub before_after_pair_integrity: f64,
    pub metadata_completeness: f64,
    pub ai_disclosure_compliance: f64,
    pub epistemic: f64,
    pub reproducibility: f64,
    pub novelty: f64,
    pub reliability: f64,
    pub metadata_policy: f64,
    /// When true, `mdl_gain_proxy` / `delta_signal_to_noise` (or human review) supports a real advance.
    #[serde(default)]
    pub meaningful_advance: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorthinessDecision {
    Publish,
    AskForEvidence,
    AbstainDoNotPublish,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorthinessEvaluation {
    pub decision: WorthinessDecision,
    pub decision_label: String,
    pub worthiness_score: f64,
    pub hard_metrics_ok: bool,
    pub reasons: Vec<String>,
}

/// Parse the worthiness contract YAML (repo file body).
pub fn load_contract_from_str(yaml: &str) -> Result<PublicationWorthinessContract> {
    serde_yaml::from_str(yaml).context("parse publication worthiness YAML")
}

/// Structural checks beyond JSON Schema (weights, ordering).
pub fn validate_contract_invariants(c: &PublicationWorthinessContract) -> Result<()> {
    let sum = c.weights.epistemic
        + c.weights.reproducibility
        + c.weights.novelty
        + c.weights.reliability
        + c.weights.metadata_policy;
    if (sum - 1.0).abs() > 1e-5 {
        return Err(anyhow!(
            "weights must sum to 1.0 (got {sum}; epistemic={}, repro={}, novelty={}, reliability={}, metadata_policy={})",
            c.weights.epistemic,
            c.weights.reproducibility,
            c.weights.novelty,
            c.weights.reliability,
            c.weights.metadata_policy
        ));
    }
    if c.thresholds.publish_score_min <= c.thresholds.abstain_score_max {
        return Err(anyhow!(
            "publish_score_min ({}) must be greater than abstain_score_max ({})",
            c.thresholds.publish_score_min,
            c.thresholds.abstain_score_max
        ));
    }
    for (name, v) in [
        (
            "claim_evidence_coverage_min",
            c.thresholds.claim_evidence_coverage_min,
        ),
        (
            "artifact_replayability_min",
            c.thresholds.artifact_replayability_min,
        ),
        (
            "before_after_pair_integrity_min",
            c.thresholds.before_after_pair_integrity_min,
        ),
        (
            "metadata_completeness_min",
            c.thresholds.metadata_completeness_min,
        ),
        (
            "ai_disclosure_compliance_exact",
            c.thresholds.ai_disclosure_compliance_exact,
        ),
        ("publish_score_min", c.thresholds.publish_score_min),
        ("abstain_score_max", c.thresholds.abstain_score_max),
    ] {
        range_01(name, v)?;
    }
    for (name, v) in [
        ("weight.epistemic", c.weights.epistemic),
        ("weight.reproducibility", c.weights.reproducibility),
        ("weight.novelty", c.weights.novelty),
        ("weight.reliability", c.weights.reliability),
        ("weight.metadata_policy", c.weights.metadata_policy),
    ] {
        range_01(name, v)?;
    }
    Ok(())
}

fn range_01(name: &str, v: f64) -> Result<()> {
    if !(0.0..=1.0).contains(&v) {
        return Err(anyhow!("{name} must be in [0,1] (got {v})"));
    }
    Ok(())
}

fn label_for(decision: WorthinessDecision, c: &PublicationWorthinessContract) -> String {
    match decision {
        WorthinessDecision::Publish => c.decision_labels.publish.clone(),
        WorthinessDecision::AskForEvidence => c.decision_labels.ask_for_evidence.clone(),
        WorthinessDecision::AbstainDoNotPublish => c.decision_labels.abstain_do_not_publish.clone(),
    }
}

/// Apply the default rubric: red lines and low aggregate abstain; metric floors gate publish; else ask.
pub fn evaluate_worthiness(
    c: &PublicationWorthinessContract,
    inputs: &WorthinessInputs,
) -> WorthinessEvaluation {
    let mut reasons: Vec<String> = Vec::new();

    let enabled_ids: std::collections::HashSet<&str> = c
        .hard_red_lines
        .iter()
        .filter(|r| r.enabled)
        .map(|r| r.id.as_str())
        .collect();

    let mut active_violations: Vec<&str> = Vec::new();
    for id in &inputs.red_line_violation_ids {
        if enabled_ids.contains(id.as_str()) {
            active_violations.push(id.as_str());
        }
    }
    if !active_violations.is_empty() {
        reasons.push(format!(
            "enabled hard red-line violations: {}",
            active_violations.join(", ")
        ));
        return WorthinessEvaluation {
            decision: WorthinessDecision::AbstainDoNotPublish,
            decision_label: label_for(WorthinessDecision::AbstainDoNotPublish, c),
            worthiness_score: aggregate_score(c, inputs),
            hard_metrics_ok: hard_metrics_ok(c, inputs),
            reasons,
        };
    }

    if inputs.repeated_unresolved_contradiction {
        reasons.push("repeated_unresolved_contradiction".to_string());
        return WorthinessEvaluation {
            decision: WorthinessDecision::AbstainDoNotPublish,
            decision_label: label_for(WorthinessDecision::AbstainDoNotPublish, c),
            worthiness_score: aggregate_score(c, inputs),
            hard_metrics_ok: hard_metrics_ok(c, inputs),
            reasons,
        };
    }

    let score = aggregate_score(c, inputs);
    if score < c.thresholds.abstain_score_max {
        reasons.push(format!(
            "worthiness_score {score:.4} < abstain_score_max {}",
            c.thresholds.abstain_score_max
        ));
        return WorthinessEvaluation {
            decision: WorthinessDecision::AbstainDoNotPublish,
            decision_label: label_for(WorthinessDecision::AbstainDoNotPublish, c),
            worthiness_score: score,
            hard_metrics_ok: hard_metrics_ok(c, inputs),
            reasons,
        };
    }

    let hard_ok = hard_metrics_ok(c, inputs);
    if !hard_ok {
        reasons.push("one_or_more_hard_metric_minimums_not_met".to_string());
        return WorthinessEvaluation {
            decision: WorthinessDecision::AskForEvidence,
            decision_label: label_for(WorthinessDecision::AskForEvidence, c),
            worthiness_score: score,
            hard_metrics_ok: false,
            reasons,
        };
    }

    if score >= c.thresholds.publish_score_min && inputs.meaningful_advance {
        reasons.push("hard_metrics_ok_and_publish_score_with_meaningful_advance".to_string());
        WorthinessEvaluation {
            decision: WorthinessDecision::Publish,
            decision_label: label_for(WorthinessDecision::Publish, c),
            worthiness_score: score,
            hard_metrics_ok: true,
            reasons,
        }
    } else {
        if score < c.thresholds.publish_score_min {
            reasons.push(format!(
                "worthiness_score {score:.4} < publish_score_min {}",
                c.thresholds.publish_score_min
            ));
        }
        if !inputs.meaningful_advance {
            reasons.push("meaningful_advance_required_for_publish".to_string());
        }
        WorthinessEvaluation {
            decision: WorthinessDecision::AskForEvidence,
            decision_label: label_for(WorthinessDecision::AskForEvidence, c),
            worthiness_score: score,
            hard_metrics_ok: true,
            reasons,
        }
    }
}

fn hard_metrics_ok(c: &PublicationWorthinessContract, inputs: &WorthinessInputs) -> bool {
    inputs.claim_evidence_coverage >= c.thresholds.claim_evidence_coverage_min
        && effective_replayability(inputs) >= c.thresholds.artifact_replayability_min
        && inputs.before_after_pair_integrity >= c.thresholds.before_after_pair_integrity_min
        && inputs.metadata_completeness >= c.thresholds.metadata_completeness_min
        && (inputs.ai_disclosure_compliance - c.thresholds.ai_disclosure_compliance_exact).abs()
            < 1e-9
}

/// The replayability value the worthiness rubric should actually gate on:
/// the measured value when `vox-replay-runner` has populated it, otherwise
/// the operator-declared value.
pub fn effective_replayability(inputs: &WorthinessInputs) -> f64 {
    inputs
        .artifact_replayability_measured
        .unwrap_or(inputs.artifact_replayability)
}

fn aggregate_score(c: &PublicationWorthinessContract, inputs: &WorthinessInputs) -> f64 {
    c.weights.epistemic * inputs.epistemic
        + c.weights.reproducibility * inputs.reproducibility
        + c.weights.novelty * inputs.novelty
        + c.weights.reliability * inputs.reliability
        + c.weights.metadata_policy * inputs.metadata_policy
}

/// Conservative cap on [`WorthinessInputs::novelty`] from a live prior-art bundle (min of prior and heuristic proxy).
#[must_use]
pub fn apply_prior_art_to_worthiness_inputs(
    inputs: &mut WorthinessInputs,
    bundle: Option<&crate::scientia_finding_ledger::NoveltyEvidenceBundleV1>,
    heuristics: Option<&crate::scientia_heuristics::ScientiaHeuristics>,
) -> Vec<String> {
    let Some(bundle) = bundle else {
        return vec![];
    };
    if bundle.normalized_hits.is_empty() {
        return vec![];
    }
    let fallback = crate::scientia_heuristics::ScientiaHeuristics::default();
    let h = heuristics.unwrap_or(&fallback);
    let (proxy, mut out) = crate::scientia_finding_ledger::novelty_inputs_adjustment(bundle, h);
    let before = inputs.novelty;
    inputs.novelty = before.min(proxy);
    out.push(format!(
        "novelty_after_prior_art_min: before={before:.4} after={:.4}",
        inputs.novelty
    ));
    out
}

/// Advisory venue checks: map `venue_profiles.required_checks` to concrete preflight outcomes (partial).
#[must_use]
pub fn machine_venue_profile_violations(
    contract: &PublicationWorthinessContract,
    profile_id: &str,
    report: &crate::publication_preflight::PreflightReport,
) -> Vec<String> {
    let Some(vp) = contract.venue_profiles.get(profile_id) else {
        return vec![];
    };
    let mut out = Vec::new();
    for check in &vp.required_checks {
        if check.as_str() == "double_blind_anonymization" {
            let bad = report.findings.iter().any(|f| {
                f.code.starts_with("double_blind_")
                    && f.severity == crate::publication_preflight::PreflightSeverity::Error
            });
            if bad {
                out.push("venue_profile:double_blind_anonymization:not_met".to_string());
            }
        }
    }
    out
}

/// Aggregate worthiness score for [`crate::PublisherConfig::worthiness_score`] (per-channel policy floors).
///
/// Matches the orchestrator news service probe: default contract under `repo_root`, `PreflightProfile::Default`.
pub fn worthiness_score_for_publication_manifest(
    manifest: &crate::publication::PublicationManifest,
    repo_root: &Path,
) -> Result<f64> {
    let path = repo_root.join(DEFAULT_CONTRACT_REL_PATH);
    let yaml = vox_bounded_fs::read_utf8_path_capped(&path)
        .with_context(|| format!("read worthiness contract {}", path.display()))?;
    let contract = load_contract_from_str(&yaml)?;
    validate_contract_invariants(&contract)?;
    let preflight = crate::publication_preflight::run_preflight(
        manifest,
        crate::publication_preflight::PreflightProfile::Default,
    );
    let h = crate::scientia_heuristics::ScientiaHeuristics::default();
    let inputs = crate::publication_preflight::worthiness_inputs_from_manifest_and_preflight(
        manifest,
        &preflight,
        Some(&h),
    );
    Ok(evaluate_worthiness(&contract, &inputs).worthiness_score)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_contract() -> PublicationWorthinessContract {
        let yaml = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../contracts/scientia/publication-worthiness.default.yaml"
        ));
        let c = load_contract_from_str(yaml).expect("default contract");
        validate_contract_invariants(&c).expect("invariants");
        c
    }

    fn sample_inputs_publish_ready() -> WorthinessInputs {
        WorthinessInputs {
            red_line_violation_ids: vec![],
            repeated_unresolved_contradiction: false,
            claim_evidence_coverage: 0.95,
            artifact_replayability: 0.9,
            artifact_replayability_measured: None,
            before_after_pair_integrity: 0.95,
            metadata_completeness: 0.95,
            ai_disclosure_compliance: 1.0,
            epistemic: 0.9,
            reproducibility: 0.9,
            novelty: 0.88,
            reliability: 0.9,
            metadata_policy: 0.95,
            meaningful_advance: true,
        }
    }

    #[test]
    fn default_contract_loads_and_evaluates_publish() {
        let c = sample_contract();
        let r = evaluate_worthiness(&c, &sample_inputs_publish_ready());
        assert_eq!(r.decision, WorthinessDecision::Publish);
        assert!(r.hard_metrics_ok);
    }

    #[test]
    fn red_line_abstains() {
        let c = sample_contract();
        let mut i = sample_inputs_publish_ready();
        i.red_line_violation_ids = vec!["fabricated_citation".to_string()];
        let r = evaluate_worthiness(&c, &i);
        assert_eq!(r.decision, WorthinessDecision::AbstainDoNotPublish);
    }

    #[test]
    fn low_score_abstains() {
        let c = sample_contract();
        let mut i = sample_inputs_publish_ready();
        i.epistemic = 0.1;
        i.reproducibility = 0.1;
        i.novelty = 0.1;
        i.reliability = 0.1;
        i.metadata_policy = 0.1;
        let r = evaluate_worthiness(&c, &i);
        assert_eq!(r.decision, WorthinessDecision::AbstainDoNotPublish);
    }

    #[test]
    fn missing_metric_floor_asks() {
        let c = sample_contract();
        let mut i = sample_inputs_publish_ready();
        i.claim_evidence_coverage = 0.5;
        let r = evaluate_worthiness(&c, &i);
        assert_eq!(r.decision, WorthinessDecision::AskForEvidence);
    }

    #[test]
    fn no_meaningful_advance_asks() {
        let c = sample_contract();
        let mut i = sample_inputs_publish_ready();
        i.meaningful_advance = false;
        let r = evaluate_worthiness(&c, &i);
        assert_eq!(r.decision, WorthinessDecision::AskForEvidence);
    }

    // ── Phase B: effective_replayability + measured-supersedes-declared ──────

    #[test]
    fn effective_replayability_falls_back_to_declared_when_measured_is_none() {
        let mut i = sample_inputs_publish_ready();
        i.artifact_replayability = 0.42;
        i.artifact_replayability_measured = None;
        assert!((effective_replayability(&i) - 0.42).abs() < 1e-12);
    }

    #[test]
    fn effective_replayability_uses_measured_when_present() {
        let mut i = sample_inputs_publish_ready();
        i.artifact_replayability = 0.99; // operator-declared (optimistic)
        i.artifact_replayability_measured = Some(0.0); // runner: hash mismatch
        assert_eq!(effective_replayability(&i), 0.0);
    }

    #[test]
    fn measured_failure_overrides_declared_pass_in_hard_metrics() {
        let c = sample_contract();
        let mut i = sample_inputs_publish_ready();
        // Declared value is well above the contract floor.
        i.artifact_replayability = 0.95;
        // But the runner measured a hash mismatch.
        i.artifact_replayability_measured = Some(0.0);
        let r = evaluate_worthiness(&c, &i);
        assert!(
            !r.hard_metrics_ok,
            "measured 0.0 must override declared 0.95 and fail the hard-gate"
        );
        assert_ne!(r.decision, WorthinessDecision::Publish);
    }

    #[test]
    fn measured_pass_alongside_declared_pass_still_publishes() {
        let c = sample_contract();
        let mut i = sample_inputs_publish_ready();
        i.artifact_replayability = 0.9;
        i.artifact_replayability_measured = Some(1.0);
        let r = evaluate_worthiness(&c, &i);
        assert_eq!(r.decision, WorthinessDecision::Publish);
        assert!(r.hard_metrics_ok);
    }
}
