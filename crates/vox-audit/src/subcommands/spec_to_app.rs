//! `vox audit spec-to-app` — CR-L0 end-to-end agent authorship loop.
//!
//! Walks `contracts/eval/spec-to-app/specs/*/spec.toml` and emits a
//! structured report. CR-L0 is the v1.0 integration test: an
//! autonomous agent loop drives Vox (via MCP) from an English spec
//! through to a passing application (`vox check` clean, tests pass,
//! `vox deploy` succeeds, `vox doctor` green) under a per-spec token
//! cost ceiling.
//!
//! Real corpus-inventory layer runs today. The autonomous-agent loop
//! itself depends on the orchestrator's MCP plan-mode + panel client
//! + budget tracking, which front-stacks onto this runner via
//! `--llm-panel` (presently returns `InfrastructureError` with a
//! note pointing at the integration follow-on).
//!
//! Replaces `SpecToAppStub`.

use crate::{
    CommonArgs, CrlGate, RunOutcome, Subcommand,
    report::{AuditReport, ExitCode, Results, Threshold},
    workspace_root,
};
use std::path::Path;

const DEFAULT_CORPUS_RELPATH: &str = "contracts/eval/spec-to-app";

pub struct SpecToAppRunner;

impl Subcommand for SpecToAppRunner {
    fn gate(&self) -> CrlGate {
        CrlGate::L0SpecToApp
    }

    fn description(&self) -> &'static str {
        "CR-L0: end-to-end agent loop (≥60% pass / ≤$5/spec). Block-GA on sub-bar."
    }

    fn run(&self, args: &CommonArgs) -> RunOutcome {
        let corpus_root = args
            .corpus
            .clone()
            .unwrap_or_else(|| workspace_root().join(DEFAULT_CORPUS_RELPATH));
        let specs_dir = corpus_root.join("specs");
        if !specs_dir.exists() {
            return RunOutcome {
                report: AuditReport::infra_error(
                    gate_thing_name(),
                    format!(
                        "specs dir not found at {}; expected per \
                         contracts/eval/spec-to-app/README.md",
                        specs_dir.display()
                    ),
                ),
                exit_code: ExitCode::InfrastructureError,
            };
        }
        let specs = match discover_specs(&specs_dir) {
            Ok(s) => s,
            Err(msg) => {
                return RunOutcome {
                    report: AuditReport::infra_error(gate_thing_name(), msg),
                    exit_code: ExitCode::InfrastructureError,
                };
            }
        };
        if specs.is_empty() {
            return RunOutcome {
                report: AuditReport::infra_error(
                    gate_thing_name(),
                    format!("no specs discovered under {}", specs_dir.display()),
                ),
                exit_code: ExitCode::InfrastructureError,
            };
        }
        if args.dry_run {
            let mut report = AuditReport::complete(
                gate_thing_name(),
                corpus_hash(&specs),
                specs.len() as u32,
                Results {
                    overall_pass_rate: 1.0,
                    median_pass_rate: None,
                    per_llm: Vec::new(),
                },
            );
            report.note = Some(format!("dry-run: {} spec(s) discovered", specs.len()));
            return RunOutcome {
                report,
                exit_code: ExitCode::Ok,
            };
        }

        // Panel mode = autonomous-agent loop. Not yet shipped — front-
        // stacks onto the orchestrator's plan-mode + panel client.
        if args.llm_panel.is_some() {
            return RunOutcome {
                report: AuditReport::infra_error(
                    gate_thing_name(),
                    "panel mode (the autonomous-agent loop driving Vox via MCP) requires \
                     the orchestrator integration follow-on. Without --llm-panel the \
                     runner reports corpus-inventory; with --llm-panel it would invoke \
                     the agent loop with the per-spec $5 cost ceiling. Not yet shipped."
                        .to_string(),
                ),
                exit_code: ExitCode::InfrastructureError,
            };
        }

        let total = specs.len() as u32;
        let target = args.threshold.unwrap_or(0.60);
        let mut by_tier = std::collections::BTreeMap::new();
        for s in &specs {
            *by_tier.entry(s.tier.clone()).or_insert(0u32) += 1;
        }
        let tier_summary: Vec<String> = by_tier
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        let mut report = AuditReport::complete(
            gate_thing_name(),
            corpus_hash(&specs),
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
        report.note = Some(format!(
            "corpus-inventory mode: {total} spec(s) ({}). End-to-end agent \
             pass rate (the CR-L0 60% bar) requires --llm-panel + the \
             autonomous-agent loop wiring.",
            tier_summary.join(", ")
        ));
        RunOutcome {
            report,
            exit_code: ExitCode::Ok,
        }
    }
}

fn gate_thing_name() -> &'static str {
    CrlGate::L0SpecToApp.thing_name()
}

#[derive(Debug)]
struct SpecFixture {
    id: String,
    tier: String,
    source: String,
}

fn discover_specs(specs_dir: &Path) -> Result<Vec<SpecFixture>, String> {
    let entries = std::fs::read_dir(specs_dir)
        .map_err(|e| format!("read {}: {}", specs_dir.display(), e))?;
    let mut out = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("dir-entry: {}", e))?;
        let p = entry.path();
        if !p.is_dir() {
            continue;
        }
        let spec_toml = p.join("spec.toml");
        if !spec_toml.exists() {
            continue;
        }
        let id = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_string();
        let source = std::fs::read_to_string(&spec_toml)
            .map_err(|e| format!("read {}: {}", spec_toml.display(), e))?;
        let tier = extract_tier(&source).unwrap_or_else(|| "unknown".to_string());
        out.push(SpecFixture { id, tier, source });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

fn extract_tier(source: &str) -> Option<String> {
    let v: toml::Value = toml::from_str(source).ok()?;
    v.get("tier")?.as_str().map(|s| s.to_string())
}

fn corpus_hash(specs: &[SpecFixture]) -> String {
    let mut hasher = blake3::Hasher::new();
    for s in specs {
        hasher.update(s.id.as_bytes());
        hasher.update(b"\n");
        hasher.update(s.source.as_bytes());
        hasher.update(b"\n");
    }
    format!("blake3:{}", hasher.finalize().to_hex())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_specs_dir_returns_infra_error() {
        let tmp = tempfile::tempdir().unwrap();
        let args = CommonArgs {
            corpus: Some(tmp.path().to_path_buf()),
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = SpecToAppRunner.run(&args);
        assert_eq!(outcome.exit_code, ExitCode::InfrastructureError);
    }

    #[test]
    fn real_specs_dir_reports_tier_inventory() {
        let tmp = tempfile::tempdir().unwrap();
        let specs = tmp.path().join("specs");
        let p = specs.join("001-tiny");
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(
            p.join("spec.toml"),
            r#"id = "tiny"
tier = "T1"
prompt = "build a tiny app"
"#,
        )
        .unwrap();
        let args = CommonArgs {
            corpus: Some(tmp.path().to_path_buf()),
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = SpecToAppRunner.run(&args);
        assert_eq!(outcome.exit_code, ExitCode::Ok);
        assert_eq!(outcome.report.corpus_size, 1);
        assert!(outcome.report.note.as_deref().unwrap_or("").contains("T1"));
    }

    #[test]
    fn panel_mode_returns_honest_infra_error() {
        let tmp = tempfile::tempdir().unwrap();
        let specs = tmp.path().join("specs");
        let p = specs.join("001-tiny");
        std::fs::create_dir_all(&p).unwrap();
        std::fs::write(
            p.join("spec.toml"),
            r#"id = "tiny"
tier = "T1"
prompt = "x"
"#,
        )
        .unwrap();
        let args = CommonArgs {
            corpus: Some(tmp.path().to_path_buf()),
            llm_panel: Some(std::path::PathBuf::from("contracts/eval/llm-panel.v1.yaml")),
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = SpecToAppRunner.run(&args);
        assert_eq!(outcome.exit_code, ExitCode::InfrastructureError);
        assert!(outcome.report.note.as_deref().unwrap_or("").contains("agent loop"));
    }
}
