//! Core umbrella gates: `arch`, `code`, `retirement` (AGENTS.md §`vox audit` umbrella).
//!
//! Distinct from CR-L `--gate all` (release-criteria registry). Use
//! `vox audit core` or `vox audit --gate core` for this trio.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;
use vox_code_audit::{OutputFormat, Severity, ToestubConfig, ToestubEngine, ToestubRunMode};

use crate::subcommands::retirement::RetirementSubcommand;
use crate::{
    CommonArgs, Subcommand,
    report::{ExitCode, ReportFormat},
};

/// Outcome for one core umbrella gate.
#[derive(Debug, Clone, Serialize)]
pub struct CoreGateResult {
    pub gate: &'static str,
    pub ok: bool,
    pub exit_code: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Summary JSON envelope for `vox audit core` / `--gate core`.
#[derive(Debug, Clone, Serialize)]
pub struct CoreGatesSummary {
    pub schema_version: u32,
    pub gates: Vec<CoreGateResult>,
    pub ok: bool,
}

/// Run `vox-arch-check` via cargo (same command as CI `arch-check` target).
pub fn run_arch_gate(root: &Path) -> CoreGateResult {
    let status = Command::new("cargo")
        .args(["run", "-q", "-p", "vox-arch-check"])
        .current_dir(root)
        .status();
    match status {
        Ok(s) => {
            let code = s.code().unwrap_or(1);
            CoreGateResult {
                gate: "arch",
                ok: s.success(),
                exit_code: code,
                detail: None,
            }
        }
        Err(e) => CoreGateResult {
            gate: "arch",
            ok: false,
            exit_code: 1,
            detail: Some(format!("spawn vox-arch-check: {e}")),
        },
    }
}

/// Run `vox-code-audit` / TOESTUB over the workspace (legacy exit policy).
pub fn run_code_gate(root: &Path) -> CoreGateResult {
    let config = ToestubConfig {
        roots: vec![root.to_path_buf()],
        min_severity: Severity::Warning,
        run_mode: ToestubRunMode::Legacy,
        format: OutputFormat::Json,
        ..ToestubConfig::default()
    };
    let engine = ToestubEngine::new(config);
    let (result, _) = engine.run_and_report();
    let ok = !result.should_fail_build(ToestubRunMode::Legacy);
    let blocking = result
        .findings
        .iter()
        .filter(|f| f.severity >= Severity::Error)
        .count();
    CoreGateResult {
        gate: "code",
        ok,
        exit_code: if ok { 0 } else { 1 },
        detail: Some(format!("{blocking} error/critical finding(s)")),
    }
}

/// Run ONLY the silent-drop detectors (catch-all-swallow + cross-crate-dup) over the
/// workspace and fail if ANY of their findings survive the optional grandfather
/// baseline. Severity stays Info (the human `vox audit code` report is unchanged); the
/// gate is count-based (no severity bump). Pass a committed
/// `contracts/toestub/silent-drop-baseline.v1.json` as `baseline` to grandfather
/// pre-existing sites so only NEW silent-drops trip the gate. Findings are filtered to
/// the two rule ids explicitly, so unrelated batch detectors cannot pollute the count.
/// (Task 2.6 / R9.)
pub fn run_silent_drop_gate(root: &Path, baseline: Option<PathBuf>) -> CoreGateResult {
    const RULES: [&str; 2] = ["vox/catch-all-swallow", "arch/cross-crate-dup"];
    let config = ToestubConfig {
        roots: vec![root.to_path_buf()],
        min_severity: Severity::Info,
        run_mode: ToestubRunMode::Audit,
        rule_filter: Some(RULES.iter().map(|s| s.to_string()).collect()),
        suppression_path: baseline,
        format: OutputFormat::Json,
        ..ToestubConfig::default()
    };
    let engine = ToestubEngine::new(config);
    let (result, _) = engine.run_and_report();
    let remaining = result
        .findings
        .iter()
        .filter(|f| RULES.contains(&f.rule_id.as_str()))
        .count();
    let ok = remaining == 0;
    CoreGateResult {
        gate: "silent-drop",
        ok,
        exit_code: if ok { 0 } else { 1 },
        detail: Some(format!(
            "{remaining} silent-drop finding(s) beyond baseline"
        )),
    }
}

/// Run ONLY the `weak_test` detector (touch-test anti-patterns) over the workspace,
/// including test trees, and fail if any finding survives the optional grandfather
/// baseline. Like the silent-drop gate this is count-based and severity-neutral: it
/// guards against NEW touch-tests (e.g. in Phase-3 coverage waves) without forcing a
/// rewrite of the grandfathered existing suite. (Task 2.6 sibling / weak_test gate.)
pub fn run_weak_test_gate(root: &Path, baseline: Option<PathBuf>) -> CoreGateResult {
    let config = ToestubConfig {
        roots: vec![root.to_path_buf()],
        min_severity: Severity::Info,
        run_mode: ToestubRunMode::Audit,
        rule_filter: Some(vec!["weak_test".to_string()]),
        tests_mode: vox_code_audit::run_context::ToestubTestsMode::Include,
        suppression_path: baseline,
        format: OutputFormat::Json,
        ..ToestubConfig::default()
    };
    let engine = ToestubEngine::new(config);
    let (result, _) = engine.run_and_report();
    let remaining = result
        .findings
        .iter()
        .filter(|f| f.rule_id == "weak_test")
        .count();
    let ok = remaining == 0;
    CoreGateResult {
        gate: "weak-test",
        ok,
        exit_code: if ok { 0 } else { 1 },
        detail: Some(format!("{remaining} weak-test finding(s) beyond baseline")),
    }
}

/// Run CR-L6 retirement parity via the existing registry subcommand.
pub fn run_retirement_gate() -> CoreGateResult {
    let args = CommonArgs {
        format: ReportFormat::Json,
        baseline: None,
        threshold: None,
        corpus: None,
        llm_panel: None,
        dry_run: false,
        write_canonical_report: false,
    };
    let outcome = RetirementSubcommand.run(&args);
    let ok = outcome.exit_code == ExitCode::Ok;
    CoreGateResult {
        gate: "retirement",
        ok,
        exit_code: outcome.exit_code.as_i32(),
        detail: None,
    }
}

/// Run arch → code → retirement and return an aggregate summary.
pub fn run_core_all(root: &Path) -> CoreGatesSummary {
    let root = workspace_root_or(root);
    let gates = vec![
        run_arch_gate(&root),
        run_code_gate(&root),
        run_retirement_gate(),
    ];
    let ok = gates.iter().all(|g| g.ok);
    CoreGatesSummary {
        schema_version: 1,
        gates,
        ok,
    }
}

fn workspace_root_or(fallback: &Path) -> PathBuf {
    let root = crate::workspace_root();
    if root.join("Cargo.toml").exists() {
        root
    } else {
        fallback.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_summary_serializes_schema_version() {
        let summary = CoreGatesSummary {
            schema_version: 1,
            gates: vec![CoreGateResult {
                gate: "arch",
                ok: true,
                exit_code: 0,
                detail: None,
            }],
            ok: true,
        };
        let json = serde_json::to_string(&summary).expect("json");
        assert!(json.contains("\"schema_version\":1"));
        assert!(json.contains("\"gate\":\"arch\""));
    }
}
