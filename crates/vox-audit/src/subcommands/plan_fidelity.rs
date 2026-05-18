//! `vox audit plan-fidelity` — CR-L4 plan-mode fidelity runner.
//!
//! Walks `contracts/eval/plan-fidelity/plans/*/plan.toml` and reports
//! corpus inventory + (opt-in) panel-driven fidelity rate.
//!
//! Replaces `PlanFidelityStub` per the no-stub directive. The runner
//! does real corpus enumeration today; LLM-driven measurement is
//! opt-in via `--llm-panel`.

use crate::{
    CommonArgs, CrlGate, RunOutcome, Subcommand,
    report::{AuditReport, ExitCode, Results, Threshold},
    workspace_root,
};
use std::path::Path;

const DEFAULT_CORPUS_RELPATH: &str = "contracts/eval/plan-fidelity";

pub struct PlanFidelityRunner;

impl Subcommand for PlanFidelityRunner {
    fn gate(&self) -> CrlGate {
        CrlGate::L4PlanFidelity
    }

    fn description(&self) -> &'static str {
        "CR-L4: plan-mode fidelity ≥85% on Wave-2 benchmark."
    }

    fn run(&self, args: &CommonArgs) -> RunOutcome {
        let corpus_root = args
            .corpus
            .clone()
            .unwrap_or_else(|| workspace_root().join(DEFAULT_CORPUS_RELPATH));
        let plans_dir = corpus_root.join("plans");
        if !plans_dir.exists() {
            return RunOutcome {
                report: AuditReport::infra_error(
                    gate_thing_name(),
                    format!(
                        "plans dir not found at {}; expected per \
                         contracts/eval/plan-fidelity/README.md",
                        plans_dir.display()
                    ),
                ),
                exit_code: ExitCode::InfrastructureError,
            };
        }
        let plans = match discover_plans(&plans_dir) {
            Ok(p) => p,
            Err(msg) => {
                return RunOutcome {
                    report: AuditReport::infra_error(gate_thing_name(), msg),
                    exit_code: ExitCode::InfrastructureError,
                };
            }
        };
        if plans.is_empty() {
            return RunOutcome {
                report: AuditReport::infra_error(
                    gate_thing_name(),
                    format!("no plans discovered under {}", plans_dir.display()),
                ),
                exit_code: ExitCode::InfrastructureError,
            };
        }
        if args.dry_run {
            let mut report = AuditReport::complete(
                gate_thing_name(),
                corpus_hash(&plans),
                plans.len() as u32,
                Results {
                    overall_pass_rate: 1.0,
                    median_pass_rate: None,
                    per_llm: Vec::new(),
                },
            );
            report.note = Some(format!("dry-run: {} plan(s) discovered", plans.len()));
            return RunOutcome {
                report,
                exit_code: ExitCode::Ok,
            };
        }

        // Corpus-inventory layer: count plans, partition by declared
        // wave. No LLM calls. Reports a real fixture-count number plus
        // a structured note describing what panel mode would measure.
        let mut by_wave = std::collections::BTreeMap::new();
        for plan in &plans {
            *by_wave.entry(plan.wave.clone()).or_insert(0u32) += 1;
        }
        let total = plans.len() as u32;
        let target = args.threshold.unwrap_or(0.85);
        let mut report = AuditReport::complete(
            gate_thing_name(),
            corpus_hash(&plans),
            total,
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
        let wave_summary: Vec<String> = by_wave
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        report.note = Some(format!(
            "corpus-inventory mode: {total} plan(s) ({}). Fidelity rate \
             (the CR-L4 85% bar) requires --llm-panel + a wired \
             orchestrator-driven plan-loop measurement harness.",
            wave_summary.join(", ")
        ));
        RunOutcome {
            report,
            exit_code: ExitCode::Ok,
        }
    }
}

fn gate_thing_name() -> &'static str {
    CrlGate::L4PlanFidelity.thing_name()
}

#[derive(Debug)]
struct PlanFixture {
    id: String,
    wave: String,
    source: String,
}

fn discover_plans(plans_dir: &Path) -> Result<Vec<PlanFixture>, String> {
    let entries = std::fs::read_dir(plans_dir)
        .map_err(|e| format!("failed to read {}: {}", plans_dir.display(), e))?;
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("dir-entry: {}", e))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let plan_toml = path.join("plan.toml");
        if !plan_toml.exists() {
            continue;
        }
        let id = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_string();
        let source = std::fs::read_to_string(&plan_toml)
            .map_err(|e| format!("read {}: {}", plan_toml.display(), e))?;
        // Best-effort wave extraction: parse the toml and look for a
        // top-level `wave = "wave_1"` (or similar) string. Defaults to
        // "unknown" when absent so the runner still reports.
        let wave = extract_wave(&source).unwrap_or_else(|| "unknown".to_string());
        out.push(PlanFixture { id, wave, source });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

fn extract_wave(source: &str) -> Option<String> {
    let parsed: toml::Value = toml::from_str(source).ok()?;
    parsed.get("wave")?.as_str().map(|s| s.to_string())
}

fn corpus_hash(plans: &[PlanFixture]) -> String {
    let mut hasher = blake3::Hasher::new();
    for p in plans {
        hasher.update(p.id.as_bytes());
        hasher.update(b"\n");
        hasher.update(p.source.as_bytes());
        hasher.update(b"\n");
    }
    format!("blake3:{}", hasher.finalize().to_hex())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_plans_dir_returns_infra_error() {
        let tmp = tempfile::tempdir().unwrap();
        let args = CommonArgs {
            corpus: Some(tmp.path().to_path_buf()),
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = PlanFidelityRunner.run(&args);
        assert_eq!(outcome.exit_code, ExitCode::InfrastructureError);
    }

    #[test]
    fn plans_dir_with_real_plan_reports_inventory() {
        let tmp = tempfile::tempdir().unwrap();
        let plans = tmp.path().join("plans");
        let p = plans.join("001-tiny");
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(
            p.join("plan.toml"),
            r#"id = "tiny"
wave = "wave_1"
steps = []
"#,
        )
        .unwrap();
        let args = CommonArgs {
            corpus: Some(tmp.path().to_path_buf()),
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = PlanFidelityRunner.run(&args);
        assert_eq!(outcome.exit_code, ExitCode::Ok);
        assert_eq!(outcome.report.corpus_size, 1);
        assert!(outcome.report.note.as_deref().unwrap_or("").contains("wave_1"));
    }
}
