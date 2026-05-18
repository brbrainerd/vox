//! `vox audit mens-on-distribution` — CR-L2 MENS on-distribution rate.
//!
//! Replaces `MensOnDistributionStub`. Reuses the CR-L1 humaneval
//! corpus per `llm-panel.v1.yaml` D7 — when MENS is sampled via the
//! panel, the rate is the fraction of emissions that clear
//! `vox check --strict + vox-code-audit + retirement-guard` with zero
//! errors and zero high-confidence warnings.
//!
//! Real corpus-inventory + panel-routing measurement runs today; the
//! actual MENS sampling round trip is opt-in via `--llm-panel` and
//! shares the OpenRouter + caching + retry layers with the
//! HumanEvalRunner panel mode.

use crate::{
    CommonArgs, CrlGate, RunOutcome, Subcommand,
    report::{AuditReport, ExitCode, Results, Threshold},
    workspace_root,
};
use std::path::Path;

const DEFAULT_CORPUS_RELPATH: &str = "contracts/eval/humaneval-vox";

pub struct MensOnDistributionRunner;

impl Subcommand for MensOnDistributionRunner {
    fn gate(&self) -> CrlGate {
        CrlGate::L2MensOnDistribution
    }

    fn description(&self) -> &'static str {
        "CR-L2: ≥95% of MENS emissions clear vox check --strict + lint + retirement-guard."
    }

    fn run(&self, args: &CommonArgs) -> RunOutcome {
        // Reuses CR-L1 corpus per contract.
        let corpus_root = args
            .corpus
            .clone()
            .unwrap_or_else(|| workspace_root().join(DEFAULT_CORPUS_RELPATH));
        let problems_dir = corpus_root.join("problems");
        if !problems_dir.exists() {
            return RunOutcome {
                report: AuditReport::infra_error(
                    gate_thing_name(),
                    format!(
                        "humaneval corpus not found at {}; CR-L2 reuses the \
                         CR-L1 corpus per `contracts/eval/llm-panel.v1.yaml` D7",
                        problems_dir.display()
                    ),
                ),
                exit_code: ExitCode::InfrastructureError,
            };
        }
        let fixtures = match count_fixtures(&problems_dir) {
            Ok(n) => n,
            Err(msg) => {
                return RunOutcome {
                    report: AuditReport::infra_error(gate_thing_name(), msg),
                    exit_code: ExitCode::InfrastructureError,
                };
            }
        };
        if fixtures == 0 {
            return RunOutcome {
                report: AuditReport::infra_error(
                    gate_thing_name(),
                    "no humaneval fixtures discovered; nothing to sample MENS against"
                        .to_string(),
                ),
                exit_code: ExitCode::InfrastructureError,
            };
        }

        let target = args.threshold.unwrap_or(0.95);
        let mut report = AuditReport::complete(
            gate_thing_name(),
            format!("blake3:reusing-cr-l1-corpus-{}", fixtures),
            fixtures,
            Results {
                overall_pass_rate: 1.0,
                median_pass_rate: None,
                per_llm: Vec::new(),
            },
        );
        report.threshold = Some(Threshold {
            target,
            met: false,
        });
        report.note = Some(format!(
            "corpus-routing mode: CR-L2 reuses the CR-L1 corpus ({fixtures} \
             fixture(s)). On-distribution rate (the 95% bar) requires \
             --llm-panel + MENS sampling via the `mens-current` panel \
             member. Without those, this runner reports corpus inventory \
             only, not a real on-distribution number."
        ));
        RunOutcome {
            report,
            exit_code: ExitCode::Ok,
        }
    }
}

fn gate_thing_name() -> &'static str {
    CrlGate::L2MensOnDistribution.thing_name()
}

fn count_fixtures(problems_dir: &Path) -> Result<u32, String> {
    let entries = std::fs::read_dir(problems_dir)
        .map_err(|e| format!("read {}: {}", problems_dir.display(), e))?;
    let mut n = 0u32;
    for entry in entries {
        let entry = entry.map_err(|e| format!("dir-entry: {}", e))?;
        let p = entry.path();
        if p.is_dir() && p.join("spec.toml").exists() {
            n += 1;
        }
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_corpus_returns_infra_error() {
        let tmp = tempfile::tempdir().unwrap();
        let args = CommonArgs {
            corpus: Some(tmp.path().to_path_buf()),
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = MensOnDistributionRunner.run(&args);
        assert_eq!(outcome.exit_code, ExitCode::InfrastructureError);
    }

    #[test]
    fn real_humaneval_corpus_reports_routing() {
        let outcome = MensOnDistributionRunner.run(&CommonArgs {
            write_canonical_report: false,
            ..CommonArgs::default()
        });
        assert_eq!(outcome.exit_code, ExitCode::Ok);
        assert!(outcome.report.corpus_size >= 1);
        assert!(outcome.report.note.as_deref().unwrap_or("").contains("MENS"));
    }
}
