//! Corpus-driven CR-L stubs.
//!
//! Each subcommand in this file emits a structurally complete
//! `AuditReport` with `incomplete: true` and exit code
//! [`ExitCode::InfrastructureError`] until its measurement *harness* is
//! implemented (P2.x in the v1 LLM-target implementation plan).
//!
//! A corpus reaching `minimum-viable` status (fixtures authored) does NOT
//! retire the stub — the harness must also exist. Stubs for corpora that are
//! already `minimum-viable` (repair-corpus, plan-fidelity as of 2026-05-27)
//! continue to return `InfrastructureError` until the P2.6/P2.7 harnesses land.
//!
//! Per `contracts/ci/vox-audit-contract.v1.yaml` §exit-codes (ratified by
//! D22 2026-05-15), exit-code-2 logs telemetry and does NOT block CI.
//!
//! When a corpus harness lands (P2/P3/P4 in the implementation plan), the
//! stub here is replaced with a real implementation in its own module.

use crate::{
    CommonArgs, CrlGate, RunOutcome, Subcommand,
    report::{AuditReport, ExitCode},
};

/// Compose an infra-error outcome for a corpus-stub subcommand.
///
/// The message is intentionally status-agnostic: corpora may be `stub` (no
/// fixtures) or `minimum-viable` (fixtures authored) — both still return
/// `InfrastructureError` until the measurement harness is implemented.
fn corpus_stub_outcome(gate: CrlGate, manifest_relpath: &str) -> RunOutcome {
    let note = format!(
        "corpus stub: measurement harness not yet implemented for `{manifest}`. \
         Fixtures may be authored (check `status` field in the manifest); the \
         harness lands per implementation-plan phasing (P2.6/P2.7). \
         Re-run after the harness is implemented.",
        manifest = manifest_relpath,
    );
    RunOutcome {
        report: AuditReport::infra_error(gate.thing_name(), note),
        exit_code: ExitCode::InfrastructureError,
    }
}

// ---------------------------------------------------------------------------
// CR-L0 — spec-to-app (end-to-end agent loop, the v1.0 integration test).
// ---------------------------------------------------------------------------

pub struct SpecToAppStub;

impl Subcommand for SpecToAppStub {
    fn gate(&self) -> CrlGate {
        CrlGate::L0SpecToApp
    }

    fn description(&self) -> &'static str {
        "CR-L0: end-to-end agent loop (≥60% pass / ≤$5/spec). Block-GA on sub-bar."
    }

    fn run(&self, _args: &CommonArgs) -> RunOutcome {
        corpus_stub_outcome(self.gate(), "contracts/eval/spec-to-app/manifest.v1.yaml")
    }
}

// ---------------------------------------------------------------------------
// CR-L1 — HumanEval-Vox.
// ---------------------------------------------------------------------------

pub struct HumanEvalStub;

impl Subcommand for HumanEvalStub {
    fn gate(&self) -> CrlGate {
        CrlGate::L1HumanEval
    }

    fn description(&self) -> &'static str {
        "CR-L1: HumanEval-Vox (≥80%) on the 164-problem corpus."
    }

    fn run(&self, _args: &CommonArgs) -> RunOutcome {
        corpus_stub_outcome(self.gate(), "contracts/eval/humaneval-vox/manifest.v1.yaml")
    }
}

// ---------------------------------------------------------------------------
// CR-L2 — MENS on-distribution rate.
// ---------------------------------------------------------------------------

pub struct MensOnDistributionStub;

impl Subcommand for MensOnDistributionStub {
    fn gate(&self) -> CrlGate {
        CrlGate::L2MensOnDistribution
    }

    fn description(&self) -> &'static str {
        "CR-L2: ≥95% of MENS emissions clear vox check --strict + lint + retirement-guard."
    }

    fn run(&self, _args: &CommonArgs) -> RunOutcome {
        // Reuses CR-L1 corpus per contract §subcommands.
        corpus_stub_outcome(self.gate(), "contracts/eval/humaneval-vox/manifest.v1.yaml")
    }
}

// ---------------------------------------------------------------------------
// CR-L3 — repair corpus.
// ---------------------------------------------------------------------------

pub struct RepairCorpusStub;

impl Subcommand for RepairCorpusStub {
    fn gate(&self) -> CrlGate {
        CrlGate::L3RepairCorpus
    }

    fn description(&self) -> &'static str {
        "CR-L3: `vox repair .` reaches ≥70% project-scope success (≥90% single-file aim)."
    }

    fn run(&self, _args: &CommonArgs) -> RunOutcome {
        corpus_stub_outcome(self.gate(), "contracts/eval/repair-corpus/manifest.v1.yaml")
    }
}

// ---------------------------------------------------------------------------
// CR-L4 — plan-mode fidelity.
// ---------------------------------------------------------------------------

pub struct PlanFidelityStub;

impl Subcommand for PlanFidelityStub {
    fn gate(&self) -> CrlGate {
        CrlGate::L4PlanFidelity
    }

    fn description(&self) -> &'static str {
        "CR-L4: ≥85% Wave-2 plan success on the 50-fixture corpus."
    }

    fn run(&self, _args: &CommonArgs) -> RunOutcome {
        corpus_stub_outcome(self.gate(), "contracts/eval/plan-fidelity/manifest.v1.yaml")
    }
}

// ---------------------------------------------------------------------------
// CR-L7 — deploy CLI E2E (vox new → vox deploy → vox doctor).
// ---------------------------------------------------------------------------

pub struct DeployStub;

impl Subcommand for DeployStub {
    fn gate(&self) -> CrlGate {
        CrlGate::L7Deploy
    }

    fn description(&self) -> &'static str {
        "CR-L7: `vox new web → vox deploy → vox doctor` E2E on every Marquee fixture."
    }

    fn run(&self, _args: &CommonArgs) -> RunOutcome {
        corpus_stub_outcome(self.gate(), "contracts/marquee/manifest.v1.yaml")
    }
}

// CR-L8 (corpus-feedback) replaced its stub in P2.2 — see
// `crate::subcommands::corpus_feedback::CorpusFeedbackSubcommand`.

#[cfg(test)]
mod tests {
    use super::*;

    fn args() -> CommonArgs {
        CommonArgs {
            write_canonical_report: false,
            ..CommonArgs::default()
        }
    }

    #[test]
    fn every_stub_returns_infrastructure_error_with_incomplete_report() {
        // Note: HumanEvalStub is no longer in the registry (replaced by real
        // impl in P2.3). Only the remaining stubs are tested here.
        let stubs: Vec<Box<dyn Subcommand>> = vec![
            Box::new(SpecToAppStub),
            Box::new(MensOnDistributionStub),
            Box::new(RepairCorpusStub),
            Box::new(PlanFidelityStub),
            Box::new(DeployStub),
        ];
        for stub in stubs {
            let outcome = stub.run(&args());
            assert_eq!(
                outcome.exit_code,
                ExitCode::InfrastructureError,
                "stub for {:?} should return InfrastructureError, got {:?}",
                stub.gate(),
                outcome.exit_code,
            );
            assert!(
                outcome.report.incomplete,
                "stub for {:?} should mark report incomplete",
                stub.gate()
            );
            assert_eq!(outcome.report.thing, stub.gate().thing_name());
            assert!(
                outcome.report.note.is_some(),
                "stub for {:?} should include a note explaining why",
                stub.gate()
            );
        }
    }

    #[test]
    fn stub_thing_names_match_gate_names() {
        // HumanEvalStub retired in P2.3 — real impl lives in humaneval.rs.
        let stubs: Vec<(Box<dyn Subcommand>, &'static str)> = vec![
            (Box::new(SpecToAppStub), "spec-to-app"),
            (Box::new(MensOnDistributionStub), "mens-on-distribution"),
            (Box::new(RepairCorpusStub), "repair-corpus"),
            (Box::new(PlanFidelityStub), "plan-fidelity"),
            (Box::new(DeployStub), "deploy"),
        ];
        for (stub, expected) in stubs {
            assert_eq!(stub.gate().thing_name(), expected);
        }
    }
}
