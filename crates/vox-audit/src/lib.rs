//! # vox-audit
//!
//! Umbrella crate hosting the [`Subcommand`] trait and the canonical
//! `vox audit <thing>` registry. One subcommand per CR-L gate
//! (CR-L0..CR-L8 / CR-L-retirement) from
//! [`docs/src/architecture/v1-release-criteria.md`](../../../docs/src/architecture/v1-release-criteria.md)
//! §5, behaviorally conforming to
//! [`contracts/ci/vox-audit-contract.v1.yaml`](../../../contracts/ci/vox-audit-contract.v1.yaml).
//!
//! Council-ratified 2026-05-15 (D21 in
//! `docs/src/architecture/v1-llm-target-implementation-plan-2026.md` §8.1):
//! new top-level crate, NOT a submodule of vox-cli — vox-cli is at TOESTUB
//! sprawl limit and this concern deserves its own home.
//!
//! ## Wiring summary
//!
//! - Library API (this file + `report.rs` + `subcommands/`): callable from
//!   tests, vox-cli, and the vox-gui audit panel.
//! - CLI binary (`src/main.rs`): the user-facing `vox-audit` executable.
//! - Single planning anchor (`contracts/ci/vox-audit-contract.v1.yaml`)
//!   declares: CLI flag set, exit-code convention, telemetry namespace,
//!   per-subcommand `gate` / `corpus` / `block_ga` / `cost_metered` fields.

pub mod aggregator;
pub mod ga;
pub mod recorder;
pub mod report;
pub mod subcommands;

use report::{AuditReport, ReportFormat};

/// Release-criteria tier. Declaration order IS the GA evaluation order:
/// foundation gates are evaluated and reported before any downstream gate
/// (see CR-F0 in `docs/src/architecture/v1-release-criteria.md` and the
/// `--gate all --strict-block-ga` roll-up).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tier {
    Foundation,
    Distribution,
    Gui,
    Product,
    Tooling,
}

impl Tier {
    /// Stable lowercase tier name used in the GA snapshot rows.
    pub fn as_str(self) -> &'static str {
        match self {
            Tier::Foundation => "foundation",
            Tier::Distribution => "distribution",
            Tier::Gui => "gui",
            Tier::Product => "product",
            Tier::Tooling => "tooling",
        }
    }
}

/// CR-L gate identifier — one variant per row in
/// `contracts/ci/vox-audit-contract.v1.yaml` §subcommands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CrlGate {
    /// CR-F1 (Foundation): behavioral goldens — `// EXPECT:` stdout matches
    /// `vox run --mode interp`. First registered Foundation-tier gate.
    F1BehavioralGoldens,
    L0SpecToApp,
    L1HumanEval,
    L2MensOnDistribution,
    L3RepairCorpus,
    L4PlanFidelity,
    L5AciDefault,
    L6Retirement,
    L7Deploy,
    L8CorpusFeedback,
    /// Non-CR-L tooling gate: stdlib-coverage parity (eval/builtins.rs ↔
    /// ref-builtins-stdlib.md ↔ scripts/). Doesn't block GA, but blocks PRs
    /// that introduce three-way drift. Lives in the same registry so it
    /// reuses the canonical report shape and CI wiring. See
    /// `docs/src/architecture/vox-stdlib-gap-audit-2026-05-23.md` §10/§12.D.
    ToolingStdlibCoverage,
}

impl CrlGate {
    /// Stable name as used by `vox audit <thing>` and the report `thing` field.
    pub fn thing_name(self) -> &'static str {
        match self {
            CrlGate::F1BehavioralGoldens => "behavioral-goldens",
            CrlGate::L0SpecToApp => "spec-to-app",
            CrlGate::L1HumanEval => "humaneval",
            CrlGate::L2MensOnDistribution => "mens-on-distribution",
            CrlGate::L3RepairCorpus => "repair-corpus",
            CrlGate::L4PlanFidelity => "plan-fidelity",
            CrlGate::L5AciDefault => "aci-default",
            CrlGate::L6Retirement => "retirement",
            CrlGate::L7Deploy => "deploy",
            CrlGate::L8CorpusFeedback => "corpus-feedback",
            CrlGate::ToolingStdlibCoverage => "stdlib-coverage",
        }
    }

    /// Per-contract: does failure of this gate block GA?
    ///
    /// Tooling gates (non-CR-L) always return false — they exist to keep
    /// the corpus healthy during development, not to gate release.
    pub fn block_ga(self) -> bool {
        matches!(
            self,
            CrlGate::F1BehavioralGoldens
                | CrlGate::L0SpecToApp
                | CrlGate::L5AciDefault
                | CrlGate::L6Retirement
                | CrlGate::L7Deploy
                | CrlGate::L8CorpusFeedback
        )
    }

    /// Per-contract: does this gate meter LLM cost during measurement?
    pub fn cost_metered(self) -> bool {
        matches!(
            self,
            CrlGate::L0SpecToApp
                | CrlGate::L1HumanEval
                | CrlGate::L2MensOnDistribution
                | CrlGate::L3RepairCorpus
                | CrlGate::L4PlanFidelity
        )
    }

    /// Which release-criteria tier this gate belongs to. The CR-L gates are
    /// §3.5 Product; the stdlib-coverage gate is Tooling. Foundation /
    /// Distribution / GUI variants enter as their gates land (Phase 0+).
    pub fn tier(self) -> Tier {
        match self {
            CrlGate::F1BehavioralGoldens => Tier::Foundation,
            CrlGate::L0SpecToApp
            | CrlGate::L1HumanEval
            | CrlGate::L2MensOnDistribution
            | CrlGate::L3RepairCorpus
            | CrlGate::L4PlanFidelity
            | CrlGate::L5AciDefault
            | CrlGate::L6Retirement
            | CrlGate::L7Deploy
            | CrlGate::L8CorpusFeedback => Tier::Product,
            CrlGate::ToolingStdlibCoverage => Tier::Tooling,
        }
    }

    /// Iterator over every registered gate, in display order.
    pub fn all() -> impl Iterator<Item = CrlGate> {
        [
            CrlGate::F1BehavioralGoldens,
            CrlGate::L0SpecToApp,
            CrlGate::L1HumanEval,
            CrlGate::L2MensOnDistribution,
            CrlGate::L3RepairCorpus,
            CrlGate::L4PlanFidelity,
            CrlGate::L5AciDefault,
            CrlGate::L6Retirement,
            CrlGate::L7Deploy,
            CrlGate::L8CorpusFeedback,
            CrlGate::ToolingStdlibCoverage,
        ]
        .into_iter()
    }
}

/// Workspace-root locator: walks up from `CARGO_MANIFEST_DIR` looking for the
/// top-level Cargo.toml that contains `[workspace]`. Used by subcommands to
/// resolve contract paths.
pub fn workspace_root() -> std::path::PathBuf {
    let mut cur = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        let candidate = cur.join("Cargo.toml");
        if candidate.exists()
            && let Ok(text) = std::fs::read_to_string(&candidate)
            && text.contains("[workspace]")
        {
            return cur;
        }
        if !cur.pop() {
            // Reached filesystem root without finding workspace. Fall back to
            // CARGO_MANIFEST_DIR/.. (vox-audit lives at crates/vox-audit, so
            // ../.. is workspace).
            return std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..");
        }
    }
}

/// Common arguments shared by every subcommand.
#[derive(Debug, Clone)]
pub struct CommonArgs {
    pub format: ReportFormat,
    pub baseline: Option<std::path::PathBuf>,
    pub threshold: Option<f64>,
    pub corpus: Option<std::path::PathBuf>,
    pub llm_panel: Option<std::path::PathBuf>,
    /// If true, the runner does NOT execute the underlying measurement; it
    /// only validates the fixture / panel inputs and exits 0 on success.
    pub dry_run: bool,
    /// If true, the report is also written to
    /// `contracts/reports/<thing>/<YYYY-MM-DD>.json` (the canonical location).
    /// Defaults true; set false for ad-hoc local runs that should not pollute
    /// the report archive.
    pub write_canonical_report: bool,
}

impl Default for CommonArgs {
    fn default() -> Self {
        Self {
            format: ReportFormat::Json,
            baseline: None,
            threshold: None,
            corpus: None,
            llm_panel: None,
            dry_run: false,
            write_canonical_report: true,
        }
    }
}

/// One run outcome: the report and the exit code the binary should return.
#[derive(Debug)]
pub struct RunOutcome {
    pub report: AuditReport,
    pub exit_code: report::ExitCode,
}

/// Trait every `vox audit <thing>` subcommand implements.
///
/// The umbrella runner (`vox audit all`) iterates over registered subcommands
/// in `CrlGate::all()` order and aggregates their outcomes into a combined
/// report at `contracts/reports/audit-all/<date>.json`.
pub trait Subcommand: Send + Sync {
    /// Which CR-L gate this subcommand implements.
    fn gate(&self) -> CrlGate;

    /// One-line human-readable description (used by `--help`).
    fn description(&self) -> &'static str;

    /// Execute the audit. Implementations MUST honor `args.dry_run`, MUST
    /// produce a structurally complete `AuditReport`, and MUST NOT panic on
    /// missing inputs (return an `InfrastructureError` exit code instead).
    fn run(&self, args: &CommonArgs) -> RunOutcome;
}

/// Build the canonical registry of subcommands (9 CR-L gates + 1 tooling gate).
///
/// Order matches `CrlGate::all()` and `contracts/ci/vox-audit-contract.v1.yaml`
/// §subcommands. Tooling gates appear after the CR-L block.
pub fn registry() -> Vec<Box<dyn Subcommand>> {
    vec![
        // CR-F1 (Foundation) — first real foundation-tier gate.
        Box::new(subcommands::behavioral_goldens::BehavioralGoldensSubcommand),
        Box::new(subcommands::stubs::SpecToAppStub),
        // P2.3: CR-L1 stub replaced by real vox-check-based impl.
        Box::new(subcommands::humaneval::HumanEvalSubcommand),
        Box::new(subcommands::stubs::MensOnDistributionStub),
        Box::new(subcommands::stubs::RepairCorpusStub),
        Box::new(subcommands::stubs::PlanFidelityStub),
        Box::new(subcommands::aci_default::AciDefaultSubcommand),
        Box::new(subcommands::retirement::RetirementSubcommand),
        Box::new(subcommands::stubs::DeployStub),
        // P2.2: CR-L8 stub replaced by real aggregator-backed impl.
        Box::new(subcommands::corpus_feedback::CorpusFeedbackSubcommand),
        // Non-CR-L tooling gate (does not block GA).
        Box::new(subcommands::stdlib_coverage::StdlibCoverageSubcommand),
    ]
}

/// Resolve a [`CrlGate`] from a stable thing-name string.
pub fn gate_from_name(name: &str) -> Option<CrlGate> {
    CrlGate::all().find(|g| g.thing_name() == name)
}

/// Run a single subcommand by gate, returning its outcome.
///
/// Emits a `vox.audit.run` telemetry event for every invocation per
/// `contracts/ci/vox-audit-contract.v1.yaml` §telemetry (A11, ratified
/// 2026-05-15). `record_event!` is a no-op when no recorder is registered,
/// so the emission adds zero cost to non-instrumented runs.
pub fn run_gate(gate: CrlGate, args: &CommonArgs) -> RunOutcome {
    let started = std::time::Instant::now();
    let registered = registry();
    let sub = registered
        .into_iter()
        .find(|s| s.gate() == gate)
        .expect("CrlGate::all() and registry() must match — bug if mismatched");
    let outcome = sub.run(args);
    emit_audit_run_event(&outcome, started, /* umbrella_run */ false);
    outcome
}

/// Run every registered subcommand and aggregate their outcomes into a
/// combined report at `contracts/reports/audit-all/<date>.json`.
///
/// Worst-case exit code semantics per contract §umbrella:
/// - 0 if all sub-runs return 0
/// - 1 if any sub returns 1 and none return 2 or 3
/// - 2 if any sub returns 2 and none return 3
/// - 3 if any sub returns 3
///
/// Emits one `vox.audit.run` telemetry event per sub-gate (with
/// `umbrella_run = true`) per A11 contract compliance.
pub fn run_all(args: &CommonArgs) -> Vec<RunOutcome> {
    registry()
        .into_iter()
        .map(|sub| {
            let started = std::time::Instant::now();
            let outcome = sub.run(args);
            emit_audit_run_event(&outcome, started, /* umbrella_run */ true);
            outcome
        })
        .collect()
}

/// Run every registered gate (foundation first per CR-F0) plus the standalone
/// product binaries, fold them into a [`ga::GaSnapshot`] with
/// `blocked_by_foundation` semantics, and (when `args.write_canonical_report`)
/// write `contracts/reports/_snapshot/<UTC>.json`. This is the spine behind
/// `vox audit --gate all --strict-block-ga`.
pub fn run_ga_snapshot(args: &CommonArgs, strict_block_ga: bool) -> ga::GaSnapshot {
    use report::ExitCode;
    let mut rows: Vec<ga::GateRow> = registry()
        .into_iter()
        .map(|sub| {
            let g = sub.gate();
            // A11: every gate invocation emits a `vox.audit.run` event, exactly
            // like run_gate/run_all. The GA roll-up is the production acceptance
            // path, so it must not skip telemetry.
            let started = std::time::Instant::now();
            let outcome = sub.run(args);
            emit_audit_run_event(&outcome, started, /* umbrella_run */ true);
            ga::GateRow {
                thing: g.thing_name().to_string(),
                tier: g.tier().as_str().to_string(),
                met: outcome.exit_code == ExitCode::Ok && !outcome.report.incomplete,
                blocked_by_foundation: false,
                exit_code: outcome.exit_code.as_i32(),
                external_infra: false,
            }
        })
        .collect();
    rows.extend(ga::product_binary_gates(args));
    let snap = ga::GaSnapshot::from_rows(rows, strict_block_ga);
    if args.write_canonical_report {
        let _ = snap.write_canonical(&workspace_root());
    }
    snap
}

// ───────────────────────────────────────────────────────────────────────────
// A11 — `vox.audit.run` telemetry emission helper.
// ───────────────────────────────────────────────────────────────────────────

/// Emit one [`vox_telemetry::AuditRunEvent`] per gate invocation. Per
/// `contracts/ci/vox-audit-contract.v1.yaml` §telemetry, every `vox audit
/// <thing>` run must produce an event carrying `corpus_hash`, `outcome`,
/// `duration_seconds`, etc. Council ratified 2026-05-15 (A11).
fn emit_audit_run_event(outcome: &RunOutcome, started: std::time::Instant, umbrella_run: bool) {
    use vox_telemetry::{AuditRunEvent, TelemetryEvent, record_event};

    let outcome_label = match outcome.exit_code {
        report::ExitCode::Ok => "ok",
        report::ExitCode::BarMissed => "bar_missed",
        report::ExitCode::InfrastructureError => "infra_error",
        report::ExitCode::InvalidInput => "invalid_input",
    };
    let duration_seconds = started.elapsed().as_secs_f64();

    // `panel_version` is None for non-LLM-panel gates (retirement, aci-default,
    // corpus-feedback). For LLM-panel gates the panel pin should eventually
    // flow from `args.llm_panel` resolution; until those gates land real
    // measurement (P2.4+), the field stays None.
    let event = TelemetryEvent::AuditRun(AuditRunEvent {
        thing: outcome.report.thing.clone(),
        outcome: outcome_label.to_string(),
        corpus_hash: outcome.report.corpus_hash.clone(),
        corpus_size: outcome.report.corpus_size,
        duration_seconds,
        cumulative_cost_usd: outcome
            .report
            .results
            .per_llm
            .iter()
            .filter_map(|r| r.median_cost_usd)
            .sum(),
        unreachable_panel_member_count: outcome
            .report
            .results
            .per_llm
            .iter()
            .filter_map(|r| r.unreachable_count)
            .sum::<u32>(),
        panel_version: outcome.report.llm_panel.first().map(|m| m.version.clone()),
        umbrella_run,
        repository_id: None, // populated when CommonArgs threads a repo id (A2-style follow-on)
    });
    record_event!(&event);
}

/// Aggregate-exit-code per contract §umbrella.
pub fn aggregate_exit_code(outcomes: &[RunOutcome]) -> report::ExitCode {
    use report::ExitCode;
    let mut worst = ExitCode::Ok;
    for outcome in outcomes {
        worst = match (worst, outcome.exit_code) {
            (_, ExitCode::InvalidInput) => ExitCode::InvalidInput,
            (ExitCode::InvalidInput, _) => ExitCode::InvalidInput,
            (_, ExitCode::InfrastructureError) => ExitCode::InfrastructureError,
            (ExitCode::InfrastructureError, _) => ExitCode::InfrastructureError,
            (_, ExitCode::BarMissed) => ExitCode::BarMissed,
            (ExitCode::BarMissed, _) => ExitCode::BarMissed,
            (ExitCode::Ok, ExitCode::Ok) => ExitCode::Ok,
        };
    }
    worst
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_gates_are_ordered_foundation_first() {
        // Tier order is the enum's declaration order (derive(Ord)).
        let tiers: Vec<Tier> = CrlGate::all().map(|g| g.tier()).collect();
        let mut sorted = tiers.clone();
        sorted.sort();
        assert_eq!(
            tiers, sorted,
            "CrlGate::all() must yield gates in non-decreasing tier order \
             (foundation → distribution → gui → product → tooling); got {tiers:?}"
        );
    }

    #[test]
    fn registry_order_matches_all_order() {
        let reg_gates: Vec<CrlGate> = registry().iter().map(|s| s.gate()).collect();
        let all_gates: Vec<CrlGate> = CrlGate::all().collect();
        assert_eq!(
            reg_gates, all_gates,
            "registry() and all() must agree on order"
        );
    }

    #[test]
    fn every_gate_has_a_subcommand_in_registry() {
        let reg = registry();
        let registered_gates: std::collections::HashSet<CrlGate> =
            reg.iter().map(|s| s.gate()).collect();
        for gate in CrlGate::all() {
            assert!(
                registered_gates.contains(&gate),
                "CrlGate::{:?} (thing={}) has no registered subcommand",
                gate,
                gate.thing_name()
            );
        }
    }

    #[test]
    fn every_subcommand_in_registry_is_a_known_gate() {
        let reg = registry();
        let all_gates: std::collections::HashSet<CrlGate> = CrlGate::all().collect();
        for sub in &reg {
            assert!(
                all_gates.contains(&sub.gate()),
                "registered subcommand has unknown gate {:?}",
                sub.gate()
            );
        }
    }

    #[test]
    fn registry_size_matches_gate_count() {
        assert_eq!(registry().len(), CrlGate::all().count());
        assert_eq!(
            registry().len(),
            11,
            "1 CR-F foundation gate + 9 CR-L gates + 1 tooling gate (stdlib-coverage) expected"
        );
    }

    #[test]
    fn thing_names_are_unique() {
        let names: Vec<&'static str> = CrlGate::all().map(|g| g.thing_name()).collect();
        let unique: std::collections::HashSet<&&'static str> = names.iter().collect();
        assert_eq!(names.len(), unique.len(), "thing_name collision: {names:?}");
    }

    #[test]
    fn gate_from_name_round_trips() {
        for gate in CrlGate::all() {
            let name = gate.thing_name();
            assert_eq!(gate_from_name(name), Some(gate));
        }
        assert_eq!(gate_from_name("does-not-exist"), None);
    }

    #[test]
    fn block_ga_set_matches_contract() {
        // Per contracts/ci/vox-audit-contract.v1.yaml: L0, L5, L6, L7, L8 block GA.
        let blockers: std::collections::HashSet<CrlGate> =
            CrlGate::all().filter(|g| g.block_ga()).collect();
        let expected: std::collections::HashSet<CrlGate> = [
            CrlGate::F1BehavioralGoldens,
            CrlGate::L0SpecToApp,
            CrlGate::L5AciDefault,
            CrlGate::L6Retirement,
            CrlGate::L7Deploy,
            CrlGate::L8CorpusFeedback,
        ]
        .into_iter()
        .collect();
        assert_eq!(blockers, expected);
    }

    #[test]
    fn cost_metered_set_matches_contract() {
        // Per contract: only the LLM-driven gates (L0-L4) meter cost.
        let metered: std::collections::HashSet<CrlGate> =
            CrlGate::all().filter(|g| g.cost_metered()).collect();
        let expected: std::collections::HashSet<CrlGate> = [
            CrlGate::L0SpecToApp,
            CrlGate::L1HumanEval,
            CrlGate::L2MensOnDistribution,
            CrlGate::L3RepairCorpus,
            CrlGate::L4PlanFidelity,
        ]
        .into_iter()
        .collect();
        assert_eq!(metered, expected);
    }

    /// A11: every `run_gate` call must produce a `vox.audit.run` telemetry
    /// event. Register a buffered recorder, run a gate, assert one event
    /// with the expected fields.
    #[test]
    fn run_gate_emits_audit_run_telemetry_event() {
        use crate::recorder::BufferedRecorder;
        use std::sync::Arc;
        use vox_telemetry::{TelemetryEvent, set_global_recorder};

        // Test binary is its own OnceLock; safe to register here.
        let recorder = Arc::new(BufferedRecorder::new());
        set_global_recorder(recorder.clone());

        let args = CommonArgs {
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = run_gate(CrlGate::L6Retirement, &args);

        let events = recorder.drain();
        let audit_events: Vec<_> = events
            .iter()
            .filter_map(|e| match e {
                TelemetryEvent::AuditRun(p) => Some(p),
                _ => None,
            })
            .collect();
        assert_eq!(
            audit_events.len(),
            1,
            "exactly one AuditRun event per run_gate; got {}",
            audit_events.len()
        );
        let ev = audit_events[0];
        assert_eq!(ev.thing, "retirement");
        assert_eq!(ev.outcome, "ok"); // retirement contract is clean
        assert!(ev.duration_seconds >= 0.0);
        assert!(!ev.umbrella_run);
        // corpus_hash flows from outcome.report → event field.
        assert_eq!(ev.corpus_hash, outcome.report.corpus_hash);
        // corpus_size flows from outcome.report → event field.
        assert_eq!(ev.corpus_size, outcome.report.corpus_size);
    }

    #[test]
    fn workspace_root_contains_marker_files() {
        let root = workspace_root();
        assert!(
            root.join("Cargo.toml").exists(),
            "Cargo.toml at workspace root"
        );
        assert!(
            root.join("AGENTS.md").exists(),
            "AGENTS.md at workspace root"
        );
    }

    #[test]
    fn aggregate_exit_code_worst_case_wins() {
        use report::ExitCode;
        let mk = |c: ExitCode| RunOutcome {
            report: AuditReport::infra_error("x", "stub"),
            exit_code: c,
        };
        assert_eq!(
            aggregate_exit_code(&[mk(ExitCode::Ok), mk(ExitCode::Ok)]),
            ExitCode::Ok
        );
        assert_eq!(
            aggregate_exit_code(&[mk(ExitCode::Ok), mk(ExitCode::BarMissed)]),
            ExitCode::BarMissed
        );
        assert_eq!(
            aggregate_exit_code(&[mk(ExitCode::BarMissed), mk(ExitCode::InfrastructureError)]),
            ExitCode::InfrastructureError
        );
        assert_eq!(
            aggregate_exit_code(&[
                mk(ExitCode::InfrastructureError),
                mk(ExitCode::InvalidInput)
            ]),
            ExitCode::InvalidInput
        );
        assert_eq!(aggregate_exit_code(&[]), ExitCode::Ok);
    }
}
