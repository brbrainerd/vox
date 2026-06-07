//! `vox audit retirement` — CR-L6 retirement-guard parity gate.
//!
//! Wraps [`vox_code_audit::retirement_parity::check_parity_at_path`] and
//! emits the canonical `AuditReport` shape. Block-GA per contract: yes —
//! drift between AGENTS.md §Retired Surfaces and the wired detectors blocks
//! release.

use crate::{
    CommonArgs, CrlGate, RunOutcome, Subcommand,
    report::{AuditReport, ExitCode, Results, Threshold},
    workspace_root,
};
use vox_code_audit::retirement_parity;

/// Default contract path relative to workspace root.
const DEFAULT_CONTRACT_RELPATH: &str = "contracts/retirement/retired-surfaces.v1.yaml";

pub struct RetirementSubcommand;

impl Subcommand for RetirementSubcommand {
    fn gate(&self) -> CrlGate {
        CrlGate::L6Retirement
    }

    fn description(&self) -> &'static str {
        "CR-L6: drift between AGENTS.md §Retired Surfaces and the wired detector registry."
    }

    fn run(&self, args: &CommonArgs) -> RunOutcome {
        let contract_path = args
            .corpus
            .clone()
            .unwrap_or_else(|| workspace_root().join(DEFAULT_CONTRACT_RELPATH));

        // Dry-run: just verify the contract file exists and parses.
        if args.dry_run {
            return match std::fs::read_to_string(&contract_path)
                .map_err(|e| e.to_string())
                .and_then(|yaml| retirement_parity::check_parity(&yaml).map_err(|e| e.to_string()))
            {
                Ok(_) => RunOutcome {
                    report: AuditReport::complete(
                        gate_thing_name(),
                        "blake3:dry-run-no-hash",
                        0,
                        Results {
                            overall_pass_rate: 1.0,
                            median_pass_rate: None,
                            per_llm: Vec::new(),
                        },
                    ),
                    exit_code: ExitCode::Ok,
                },
                Err(msg) => RunOutcome {
                    report: AuditReport::infra_error(
                        gate_thing_name(),
                        format!("dry-run failed: {msg}"),
                    ),
                    exit_code: ExitCode::InvalidInput,
                },
            };
        }

        match retirement_parity::check_parity_at_path(&contract_path) {
            Ok(parity) => {
                let pass = if parity.is_clean() { 1.0 } else { 0.0 };
                let mut report = AuditReport::complete(
                    gate_thing_name(),
                    contract_hash(&contract_path),
                    parity_total_rows(&parity) as u32,
                    Results {
                        overall_pass_rate: pass,
                        median_pass_rate: None,
                        per_llm: Vec::new(),
                    },
                );
                report.threshold = Some(Threshold {
                    target: args.threshold.unwrap_or(1.0),
                    met: parity.is_clean(),
                });
                if !parity.is_clean() {
                    report.note = Some(format!("parity drift: {}", parity.summary()));
                }
                let exit_code = if parity.is_clean() {
                    ExitCode::Ok
                } else {
                    ExitCode::BarMissed
                };
                RunOutcome { report, exit_code }
            }
            Err(io_err) => RunOutcome {
                report: AuditReport::infra_error(
                    gate_thing_name(),
                    format!(
                        "failed to read retirement contract at {}: {io_err}",
                        contract_path.display()
                    ),
                ),
                exit_code: ExitCode::InfrastructureError,
            },
        }
    }
}

fn gate_thing_name() -> &'static str {
    CrlGate::L6Retirement.thing_name()
}

/// Compute a content-derived corpus hash for the given contract file.
///
/// Returns the empty-hash sentinel on read failure (callers will already have
/// surfaced the underlying error via [`ExitCode::InfrastructureError`]).
fn contract_hash(path: &std::path::Path) -> String {
    match std::fs::read(path) {
        Ok(bytes) => format!("blake3:{}", blake3::hash(&bytes).to_hex()),
        Err(_) => "blake3:unavailable".to_string(),
    }
}

/// Best-effort row count: sum of every category in the parity report.
fn parity_total_rows(report: &retirement_parity::ParityReport) -> usize {
    report.detector_rows_ok.len()
        + report.detector_rows_missing_rule.len()
        + report.detector_rows_missing_diagnostic_id.len()
        + report.cli_check_rows_ok.len()
        + report.documentation_only_rows_ok.len()
        + report.deferred_rows_ok.len()
        + report.deferred_rows_missing_milestone.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retirement_subcommand_runs_against_workspace_contract() {
        let args = CommonArgs {
            // Don't write a canonical report from inside tests.
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = RetirementSubcommand.run(&args);
        // Today's contract is clean per P1.3 work.
        assert_eq!(
            outcome.exit_code,
            ExitCode::Ok,
            "expected clean retirement parity; report note: {:?}",
            outcome.report.note
        );
        assert!(!outcome.report.incomplete);
        assert_eq!(outcome.report.thing, "retirement");
        assert!(
            outcome.report.corpus_size >= 5,
            "contract has at least the 5 detector-enforced rows"
        );
        assert!(
            outcome.report.corpus_hash.starts_with("blake3:"),
            "corpus_hash should be a content hash, got {}",
            outcome.report.corpus_hash
        );
    }

    #[test]
    fn retirement_subcommand_dry_run_returns_ok() {
        let args = CommonArgs {
            dry_run: true,
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = RetirementSubcommand.run(&args);
        assert_eq!(outcome.exit_code, ExitCode::Ok);
    }

    #[test]
    fn retirement_subcommand_with_missing_corpus_returns_infra_error() {
        let args = CommonArgs {
            corpus: Some(std::path::PathBuf::from(
                "this/path/does/not/exist/retired-surfaces.v1.yaml",
            )),
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = RetirementSubcommand.run(&args);
        assert_eq!(outcome.exit_code, ExitCode::InfrastructureError);
        assert!(outcome.report.incomplete);
    }

    #[test]
    fn retirement_subcommand_threshold_defaults_to_one() {
        let args = CommonArgs {
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = RetirementSubcommand.run(&args);
        let threshold = outcome
            .report
            .threshold
            .expect("retirement always emits a threshold");
        assert_eq!(threshold.target, 1.0);
        assert!(threshold.met);
    }
}
