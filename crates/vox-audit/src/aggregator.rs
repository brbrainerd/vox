//! CR-L8 corpus-feedback aggregator (P2.2).
//!
//! Pure function that consumes a slice of [`TelemetryEvent`]s and produces a
//! [`CorpusFeedbackReport`] matching the CR-L8 quarterly artifact shape
//! defined in
//! [`docs/src/architecture/v1-release-criteria.md`](../../../../docs/src/architecture/v1-release-criteria.md)
//! §5 CR-L8 and detailed in
//! [`docs/src/architecture/v1-llm-target-implementation-plan-2026.md`](../../../../docs/src/architecture/v1-llm-target-implementation-plan-2026.md)
//! §1.3 P2.1–P2.3.
//!
//! The aggregator is intentionally I/O-free: callers supply the event slice
//! (sourced from [`crate::recorder`] or a JSONL file on disk) and the
//! aggregator returns the canonical report shape. Council-ratified 2026-05-15
//! (CR-L8 binary gate; this is the impl that flips it from stub → real).

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use vox_telemetry::TelemetryEvent;

/// Identifier used to bucket events whose `repository_id` is `None`.
/// Surfaced as a key in `by_repository` so the field never silently drops data.
pub const UNATTRIBUTED_REPO_KEY: &str = "(unattributed)";

/// Top-level CR-L8 report shape persisted to
/// `contracts/reports/corpus-feedback/<quarter>.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CorpusFeedbackReport {
    pub schema_version: u32,
    pub measured_at: String,
    /// Total `vox.lint.finding` events observed in the window.
    pub total_lint_findings: u64,
    /// Total `vox.lint.autofix_applied` + `vox.lint.autofix_rejected` events.
    pub total_autofix_observations: u64,
    /// Total `vox.repair.outcome` events (= total repair sessions).
    pub total_repair_sessions: u64,
    /// Top firing diagnostics, ordered by `count` descending, ties broken by
    /// `rule_id` lexicographic. Capped at 50 entries per CR-L8 spec.
    pub top_50_diagnostics: Vec<DiagnosticRollup>,
    /// Repair-outcome histogram, with derived success rate.
    pub repair_outcomes: RepairOutcomeHistogram,
    /// Per-repository rollups, keyed by `repository_id`. Events whose
    /// `repository_id` is `None` are bucketed under [`UNATTRIBUTED_REPO_KEY`].
    /// `BTreeMap` for deterministic JSON ordering across runs. Added 2026-05-15
    /// (A8 follow-on to P2.2). Empty when no events carry a repo identifier
    /// — never `null` — so downstream tools can iterate without a presence check.
    #[serde(default)]
    pub by_repository: BTreeMap<String, RepoRollup>,
}

/// Per-repository slice of [`CorpusFeedbackReport`]. Carries the same shape as
/// the top-level totals but scoped to one `repository_id` bucket.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RepoRollup {
    pub total_lint_findings: u64,
    pub total_autofix_observations: u64,
    pub total_repair_sessions: u64,
    pub repair_outcomes: RepairOutcomeHistogram,
    /// Per-repo top firing diagnostics, capped at 10 (smaller than the
    /// top-level top-50 since per-repo signals are noisier).
    pub top_10_diagnostics: Vec<DiagnosticRollup>,
}

/// One row of the top-50 firing diagnostics table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DiagnosticRollup {
    pub rule_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic_id: Option<String>,
    /// Number of `LintFinding` events with this rule_id.
    pub finding_count: u64,
    /// Number of `LintAutofix(applied)` events with this rule_id.
    pub autofix_applied_count: u64,
    /// Number of `LintAutofix(rejected)` events with this rule_id.
    pub autofix_rejected_count: u64,
    /// `applied / (applied + rejected)` when the denominator is > 0; `None` when
    /// no autofix observations exist (the rule may not offer fixes, or the
    /// fix-pipeline hasn't fired yet).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autofix_applied_rate: Option<f64>,
}

/// Repair-outcome histogram per CR-L8 spec.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct RepairOutcomeHistogram {
    pub success: u64,
    pub partial: u64,
    pub abandoned: u64,
    pub infra_error: u64,
    /// Any final_state not in the canonical 4-value set is bucketed here
    /// (forward-compat: new states can be added without dropping data).
    pub other: u64,
    pub total_sessions: u64,
    /// `success / total_sessions` when total > 0.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub success_rate: Option<f64>,
}

/// Per-rule bucket during aggregation; merged into a [`DiagnosticRollup`] at the end.
#[derive(Default)]
struct PerRuleBucket {
    diagnostic_id: Option<String>,
    finding_count: u64,
    autofix_applied: u64,
    autofix_rejected: u64,
}

impl PerRuleBucket {
    fn into_diagnostic_rollup(self, rule_id: String) -> DiagnosticRollup {
        let denom = self.autofix_applied + self.autofix_rejected;
        let rate = if denom > 0 {
            Some(self.autofix_applied as f64 / denom as f64)
        } else {
            None
        };
        DiagnosticRollup {
            rule_id,
            diagnostic_id: self.diagnostic_id,
            finding_count: self.finding_count,
            autofix_applied_count: self.autofix_applied,
            autofix_rejected_count: self.autofix_rejected,
            autofix_applied_rate: rate,
        }
    }
}

/// Scratch state for one rollup bucket (top-level or per-repo).
#[derive(Default)]
struct ScratchRollup {
    rules: HashMap<String, PerRuleBucket>,
    outcomes: RepairOutcomeHistogram,
    total_lint_findings: u64,
    total_autofix_observations: u64,
    total_repair_sessions: u64,
}

impl ScratchRollup {
    fn observe(&mut self, event: &TelemetryEvent) {
        match event {
            TelemetryEvent::LintFinding(payload) => {
                self.total_lint_findings += 1;
                let entry = self.rules.entry(payload.rule_id.clone()).or_default();
                entry.finding_count += 1;
                if payload.diagnostic_id.is_some() {
                    entry.diagnostic_id.clone_from(&payload.diagnostic_id);
                }
            }
            TelemetryEvent::LintAutofix(payload) => {
                self.total_autofix_observations += 1;
                let entry = self.rules.entry(payload.rule_id.clone()).or_default();
                if payload.diagnostic_id.is_some() && entry.diagnostic_id.is_none() {
                    entry.diagnostic_id.clone_from(&payload.diagnostic_id);
                }
                match payload.outcome.as_str() {
                    "applied" => entry.autofix_applied += 1,
                    "rejected" => entry.autofix_rejected += 1,
                    _ => {} // forward-compat: unknown outcome strings observed but not rated
                }
            }
            TelemetryEvent::RepairOutcome(payload) => {
                self.total_repair_sessions += 1;
                self.outcomes.total_sessions += 1;
                match payload.final_state.as_str() {
                    "success" => self.outcomes.success += 1,
                    "partial" => self.outcomes.partial += 1,
                    "abandoned" => self.outcomes.abandoned += 1,
                    "infra_error" => self.outcomes.infra_error += 1,
                    _ => self.outcomes.other += 1,
                }
            }
            _ => {} // forward-compat: RepairAttempt and future variants skipped
        }
    }

    fn finalize_outcomes(&mut self) {
        if self.outcomes.total_sessions > 0 {
            self.outcomes.success_rate =
                Some(self.outcomes.success as f64 / self.outcomes.total_sessions as f64);
        }
    }

    /// Drain the per-rule buckets into a sorted-and-truncated `DiagnosticRollup` list.
    fn top_n_diagnostics(self, n: usize) -> Vec<DiagnosticRollup> {
        let mut rollups: Vec<DiagnosticRollup> = self
            .rules
            .into_iter()
            .map(|(rule_id, b)| b.into_diagnostic_rollup(rule_id))
            .collect();
        rollups.sort_by(|a, b| {
            b.finding_count
                .cmp(&a.finding_count)
                .then_with(|| a.rule_id.cmp(&b.rule_id))
        });
        rollups.truncate(n);
        rollups
    }
}

/// Extract `repository_id` from any of the CR-L8 event variants. Returns
/// [`UNATTRIBUTED_REPO_KEY`] when the event carries no repo, so per-repo
/// rollups never silently drop data.
fn event_repository_key(event: &TelemetryEvent) -> &str {
    match event {
        TelemetryEvent::LintFinding(p) => {
            p.repository_id.as_deref().unwrap_or(UNATTRIBUTED_REPO_KEY)
        }
        TelemetryEvent::LintAutofix(p) => {
            p.repository_id.as_deref().unwrap_or(UNATTRIBUTED_REPO_KEY)
        }
        TelemetryEvent::RepairAttempt(p) => {
            p.repository_id.as_deref().unwrap_or(UNATTRIBUTED_REPO_KEY)
        }
        TelemetryEvent::RepairOutcome(p) => {
            p.repository_id.as_deref().unwrap_or(UNATTRIBUTED_REPO_KEY)
        }
        _ => UNATTRIBUTED_REPO_KEY,
    }
}

/// Aggregate a slice of events into a [`CorpusFeedbackReport`].
///
/// Pure function: deterministic given the same event slice and the same
/// `measured_at` timestamp. `measured_at` is supplied by the caller (so tests
/// can pin it) rather than computed internally.
///
/// Produces both the workspace-wide rollup and per-`repository_id` buckets
/// (A8, ratified 2026-05-15). Events with no `repository_id` are bucketed
/// under [`UNATTRIBUTED_REPO_KEY`].
pub fn aggregate(events: &[TelemetryEvent], measured_at: &str) -> CorpusFeedbackReport {
    let mut global = ScratchRollup::default();
    let mut by_repo: HashMap<String, ScratchRollup> = HashMap::new();

    for event in events {
        global.observe(event);
        let key = event_repository_key(event).to_string();
        by_repo.entry(key).or_default().observe(event);
    }

    global.finalize_outcomes();
    let total_lint_findings = global.total_lint_findings;
    let total_autofix_observations = global.total_autofix_observations;
    let total_repair_sessions = global.total_repair_sessions;
    let repair_outcomes = global.outcomes.clone();
    let top_50 = global.top_n_diagnostics(50);

    let by_repository: BTreeMap<String, RepoRollup> = by_repo
        .into_iter()
        .map(|(repo, mut scratch)| {
            scratch.finalize_outcomes();
            let totals = (
                scratch.total_lint_findings,
                scratch.total_autofix_observations,
                scratch.total_repair_sessions,
                scratch.outcomes.clone(),
            );
            let top_10 = scratch.top_n_diagnostics(10);
            (
                repo,
                RepoRollup {
                    total_lint_findings: totals.0,
                    total_autofix_observations: totals.1,
                    total_repair_sessions: totals.2,
                    repair_outcomes: totals.3,
                    top_10_diagnostics: top_10,
                },
            )
        })
        .collect();

    CorpusFeedbackReport {
        schema_version: 1,
        measured_at: measured_at.to_string(),
        total_lint_findings,
        total_autofix_observations,
        total_repair_sessions,
        top_50_diagnostics: top_50,
        repair_outcomes,
        by_repository,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vox_telemetry::{
        LintAutofixEvent, LintFindingEvent, RepairAttemptEvent, RepairOutcomeEvent,
    };

    fn fixed_now() -> &'static str {
        "2026-05-15T12:00:00Z"
    }

    fn lint(rule: &str, diag: Option<&str>) -> TelemetryEvent {
        TelemetryEvent::LintFinding(LintFindingEvent {
            rule_id: rule.into(),
            diagnostic_id: diag.map(str::to_string),
            severity: "warning".into(),
            relative_path: "x.vox".into(),
            line: 1,
            autofix_available: true,
            confidence: Some("high".into()),
            repository_id: None,
        })
    }

    fn autofix(rule: &str, outcome: &str) -> TelemetryEvent {
        TelemetryEvent::LintAutofix(LintAutofixEvent {
            rule_id: rule.into(),
            diagnostic_id: None,
            outcome: outcome.into(),
            reason: None,
            relative_path: "x.vox".into(),
            line: 1,
            repository_id: None,
        })
    }

    fn outcome(state: &str) -> TelemetryEvent {
        TelemetryEvent::RepairOutcome(RepairOutcomeEvent {
            final_state: state.into(),
            attempts_used: 1,
            attempts_budget: 3,
            total_cost_usd: 0.0,
            total_duration_ms: 100,
            residual_diagnostics: 0,
            note: None,
            repository_id: None,
        })
    }

    fn attempt() -> TelemetryEvent {
        TelemetryEvent::RepairAttempt(RepairAttemptEvent {
            attempt_number: 1,
            diagnostics_in: 2,
            diagnostics_out: 0,
            files_touched: 1,
            cost_usd: 0.0,
            duration_ms: 100,
            panel_member_id: None,
            repository_id: None,
        })
    }

    #[test]
    fn empty_events_produce_zero_report() {
        let report = aggregate(&[], fixed_now());
        assert_eq!(report.schema_version, 1);
        assert_eq!(report.total_lint_findings, 0);
        assert_eq!(report.total_autofix_observations, 0);
        assert_eq!(report.total_repair_sessions, 0);
        assert!(report.top_50_diagnostics.is_empty());
        assert_eq!(report.repair_outcomes, RepairOutcomeHistogram::default());
    }

    #[test]
    fn counts_lint_findings_per_rule() {
        let events = vec![
            lint(
                "retired/decorator-usage",
                Some("vox/retired/decorator-usage"),
            ),
            lint(
                "retired/decorator-usage",
                Some("vox/retired/decorator-usage"),
            ),
            lint(
                "retired/decorator-usage",
                Some("vox/retired/decorator-usage"),
            ),
            lint("retired/crate-import", Some("vox/retired/crate-import")),
        ];
        let report = aggregate(&events, fixed_now());
        assert_eq!(report.total_lint_findings, 4);
        assert_eq!(report.top_50_diagnostics.len(), 2);
        // Higher count comes first.
        assert_eq!(
            report.top_50_diagnostics[0].rule_id,
            "retired/decorator-usage"
        );
        assert_eq!(report.top_50_diagnostics[0].finding_count, 3);
        assert_eq!(report.top_50_diagnostics[1].rule_id, "retired/crate-import");
        assert_eq!(report.top_50_diagnostics[1].finding_count, 1);
    }

    #[test]
    fn top_50_caps_at_50_entries() {
        // Generate 60 distinct rules each with one finding.
        let events: Vec<TelemetryEvent> = (0..60)
            .map(|i| lint(&format!("rule/{i:03}"), None))
            .collect();
        let report = aggregate(&events, fixed_now());
        assert_eq!(report.total_lint_findings, 60);
        assert_eq!(report.top_50_diagnostics.len(), 50);
    }

    #[test]
    fn autofix_rate_computed_when_observations_exist() {
        let events = vec![
            lint("R1", None),
            autofix("R1", "applied"),
            autofix("R1", "applied"),
            autofix("R1", "applied"),
            autofix("R1", "rejected"),
        ];
        let report = aggregate(&events, fixed_now());
        let r1 = &report.top_50_diagnostics[0];
        assert_eq!(r1.rule_id, "R1");
        assert_eq!(r1.autofix_applied_count, 3);
        assert_eq!(r1.autofix_rejected_count, 1);
        assert_eq!(r1.autofix_applied_rate, Some(0.75));
    }

    #[test]
    fn autofix_rate_is_none_when_no_observations() {
        let events = vec![lint("R1", None), lint("R1", None)];
        let report = aggregate(&events, fixed_now());
        assert_eq!(report.top_50_diagnostics[0].autofix_applied_rate, None);
    }

    #[test]
    fn unknown_autofix_outcome_is_counted_in_total_but_not_in_rate() {
        let events = vec![
            lint("R1", None),
            autofix("R1", "applied"),
            autofix("R1", "weird-future-outcome"),
        ];
        let report = aggregate(&events, fixed_now());
        assert_eq!(report.total_autofix_observations, 2);
        // Only "applied" counts in numerator; unknown is dropped from rate denom.
        let r1 = &report.top_50_diagnostics[0];
        assert_eq!(r1.autofix_applied_count, 1);
        assert_eq!(r1.autofix_rejected_count, 0);
        assert_eq!(r1.autofix_applied_rate, Some(1.0));
    }

    #[test]
    fn repair_outcome_histogram_counts_canonical_states() {
        let events = vec![
            outcome("success"),
            outcome("success"),
            outcome("success"),
            outcome("partial"),
            outcome("abandoned"),
            outcome("infra_error"),
        ];
        let report = aggregate(&events, fixed_now());
        assert_eq!(report.total_repair_sessions, 6);
        assert_eq!(report.repair_outcomes.success, 3);
        assert_eq!(report.repair_outcomes.partial, 1);
        assert_eq!(report.repair_outcomes.abandoned, 1);
        assert_eq!(report.repair_outcomes.infra_error, 1);
        assert_eq!(report.repair_outcomes.other, 0);
        assert_eq!(report.repair_outcomes.success_rate, Some(0.5));
    }

    #[test]
    fn repair_outcome_unknown_state_buckets_to_other() {
        let events = vec![outcome("success"), outcome("future-state-tbd")];
        let report = aggregate(&events, fixed_now());
        assert_eq!(report.repair_outcomes.success, 1);
        assert_eq!(report.repair_outcomes.other, 1);
        assert_eq!(report.repair_outcomes.total_sessions, 2);
        assert_eq!(report.repair_outcomes.success_rate, Some(0.5));
    }

    #[test]
    fn repair_attempt_events_do_not_count_as_sessions() {
        // RepairAttempt is an in-session event; only RepairOutcome closes a session.
        let events = vec![attempt(), attempt(), outcome("success")];
        let report = aggregate(&events, fixed_now());
        assert_eq!(report.total_repair_sessions, 1);
        assert_eq!(report.repair_outcomes.success, 1);
    }

    #[test]
    fn report_serializes_to_json_and_round_trips() {
        let events = vec![
            lint("R1", Some("vox/r1")),
            autofix("R1", "applied"),
            outcome("success"),
        ];
        let report = aggregate(&events, fixed_now());
        let json = serde_json::to_string(&report).expect("serialize");
        let back: CorpusFeedbackReport = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(report, back);
        // Forward-compat: optional-None fields skipped.
        assert!(!json.contains("\"success_rate\":null"));
    }

    // ─── A8: per-repository bucketing ────────────────────────────────────

    fn lint_with_repo(rule: &str, repo: Option<&str>) -> TelemetryEvent {
        TelemetryEvent::LintFinding(vox_telemetry::LintFindingEvent {
            rule_id: rule.into(),
            diagnostic_id: None,
            severity: "warning".into(),
            relative_path: "x.vox".into(),
            line: 1,
            autofix_available: false,
            confidence: None,
            repository_id: repo.map(str::to_string),
        })
    }

    fn outcome_with_repo(state: &str, repo: Option<&str>) -> TelemetryEvent {
        TelemetryEvent::RepairOutcome(vox_telemetry::RepairOutcomeEvent {
            final_state: state.into(),
            attempts_used: 1,
            attempts_budget: 3,
            total_cost_usd: 0.0,
            total_duration_ms: 0,
            residual_diagnostics: 0,
            note: None,
            repository_id: repo.map(str::to_string),
        })
    }

    #[test]
    fn by_repository_buckets_events_by_repo_id() {
        let events = vec![
            lint_with_repo("R1", Some("alpha")),
            lint_with_repo("R1", Some("alpha")),
            lint_with_repo("R2", Some("alpha")),
            lint_with_repo("R1", Some("beta")),
            outcome_with_repo("success", Some("alpha")),
            outcome_with_repo("abandoned", Some("beta")),
        ];
        let report = aggregate(&events, fixed_now());

        // Workspace-wide totals still correct.
        assert_eq!(report.total_lint_findings, 4);
        assert_eq!(report.total_repair_sessions, 2);

        // Per-repo buckets present and partitioned correctly.
        assert_eq!(report.by_repository.len(), 2);
        let alpha = report
            .by_repository
            .get("alpha")
            .expect("alpha bucket present");
        assert_eq!(alpha.total_lint_findings, 3);
        assert_eq!(alpha.total_repair_sessions, 1);
        assert_eq!(alpha.repair_outcomes.success, 1);
        assert_eq!(alpha.repair_outcomes.success_rate, Some(1.0));
        assert_eq!(alpha.top_10_diagnostics.len(), 2);
        assert_eq!(alpha.top_10_diagnostics[0].rule_id, "R1");
        assert_eq!(alpha.top_10_diagnostics[0].finding_count, 2);

        let beta = report
            .by_repository
            .get("beta")
            .expect("beta bucket present");
        assert_eq!(beta.total_lint_findings, 1);
        assert_eq!(beta.total_repair_sessions, 1);
        assert_eq!(beta.repair_outcomes.abandoned, 1);
        assert_eq!(beta.repair_outcomes.success_rate, Some(0.0));
    }

    #[test]
    fn events_without_repository_id_bucket_to_unattributed() {
        let events = vec![
            lint_with_repo("R1", None),
            lint_with_repo("R1", None),
            lint_with_repo("R1", Some("alpha")),
        ];
        let report = aggregate(&events, fixed_now());
        assert!(
            report.by_repository.contains_key(UNATTRIBUTED_REPO_KEY),
            "unattributed events must bucket to '{}', got keys {:?}",
            UNATTRIBUTED_REPO_KEY,
            report.by_repository.keys().collect::<Vec<_>>()
        );
        assert_eq!(
            report.by_repository[UNATTRIBUTED_REPO_KEY].total_lint_findings,
            2
        );
        assert_eq!(report.by_repository["alpha"].total_lint_findings, 1);
    }

    #[test]
    fn by_repository_top_10_caps_at_10_entries() {
        let mut events = Vec::new();
        for i in 0..15 {
            events.push(lint_with_repo(&format!("rule/{i:02}"), Some("alpha")));
        }
        let report = aggregate(&events, fixed_now());
        assert_eq!(report.by_repository["alpha"].total_lint_findings, 15);
        assert_eq!(report.by_repository["alpha"].top_10_diagnostics.len(), 10);
    }

    #[test]
    fn by_repository_btreemap_yields_deterministic_json_order() {
        // BTreeMap → keys serialize in sorted order. Two aggregations of the
        // same event set must produce byte-identical JSON.
        let events = vec![
            lint_with_repo("R", Some("zeta")),
            lint_with_repo("R", Some("alpha")),
            lint_with_repo("R", Some("mu")),
        ];
        let a = aggregate(&events, fixed_now());
        let b = aggregate(&events, fixed_now());
        let ja = serde_json::to_string(&a).expect("ser");
        let jb = serde_json::to_string(&b).expect("ser");
        assert_eq!(
            ja, jb,
            "aggregation must be deterministic across runs of the same inputs"
        );
        // BTreeMap key order in JSON object is iteration order = sorted.
        let alpha_idx = ja.find("\"alpha\"").expect("alpha key in JSON");
        let mu_idx = ja.find("\"mu\"").expect("mu key in JSON");
        let zeta_idx = ja.find("\"zeta\"").expect("zeta key in JSON");
        assert!(alpha_idx < mu_idx && mu_idx < zeta_idx);
    }

    #[test]
    fn deterministic_tie_break_by_rule_id() {
        // Two rules with equal counts — sort must be stable + lex-ordered.
        let events = vec![
            lint("R2", None),
            lint("R2", None),
            lint("R1", None),
            lint("R1", None),
        ];
        let report = aggregate(&events, fixed_now());
        assert_eq!(report.top_50_diagnostics[0].rule_id, "R1");
        assert_eq!(report.top_50_diagnostics[1].rule_id, "R2");
    }
}
