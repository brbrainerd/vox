//! `vox audit stdlib-coverage` — non-CR-L tooling gate for stdlib drift.
//!
//! Wraps [`vox_code_audit::stdlib_parity::check_parity_at_paths`] and emits the
//! canonical [`AuditReport`] shape. Does NOT block GA (per
//! [`CrlGate::ToolingStdlibCoverage::block_ga()`]); it exists to prevent
//! three-way drift between binary registrations, doc claims, and corpus call
//! sites during development.
//!
//! See [`docs/src/architecture/vox-stdlib-gap-audit-2026-05-23.md`](../../../../../docs/src/architecture/vox-stdlib-gap-audit-2026-05-23.md) §10 / §12.D
//! for the design.

use crate::{
    CommonArgs, CrlGate, RunOutcome, Subcommand,
    report::{AuditReport, ExitCode, Results, Threshold},
    workspace_root,
};
use vox_code_audit::stdlib_parity;

/// Default paths relative to workspace root.
const DEFAULT_BINARY_SOURCE_RELPATH: &str = "crates/vox-compiler/src/eval/builtins.rs";
const DEFAULT_DOC_RELPATH: &str = "docs/src/reference/ref-builtins-stdlib.md";
const DEFAULT_CORPUS_RELPATH: &str = "scripts";

pub struct StdlibCoverageSubcommand;

impl Subcommand for StdlibCoverageSubcommand {
    fn gate(&self) -> CrlGate {
        CrlGate::ToolingStdlibCoverage
    }

    fn description(&self) -> &'static str {
        "Tooling: three-way drift between eval/builtins.rs registrations, \
         ref-builtins-stdlib.md doc claims, and scripts/ corpus call sites."
    }

    fn run(&self, args: &CommonArgs) -> RunOutcome {
        let root = workspace_root();
        let binary_source = root.join(DEFAULT_BINARY_SOURCE_RELPATH);
        // The --corpus override targets the corpus side (scripts root); the
        // binary source and doc paths stay anchored to workspace_root because
        // they're internal to the build.
        let corpus_root = args
            .corpus
            .clone()
            .unwrap_or_else(|| root.join(DEFAULT_CORPUS_RELPATH));
        let doc = root.join(DEFAULT_DOC_RELPATH);

        // Dry-run: verify that all three sources exist and are readable.
        if args.dry_run {
            for (label, path) in [
                ("binary source", &binary_source),
                ("doc", &doc),
                ("corpus root", &corpus_root),
            ] {
                if !path.exists() {
                    return RunOutcome {
                        report: AuditReport::infra_error(
                            gate_thing_name(),
                            format!("dry-run failed: {label} not found at {}", path.display()),
                        ),
                        exit_code: ExitCode::InvalidInput,
                    };
                }
            }
            return RunOutcome {
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
            };
        }

        match stdlib_parity::check_parity_at_paths(&binary_source, &doc, &corpus_root) {
            Ok(parity) => {
                let pass = if parity.is_clean() { 1.0 } else { 0.0 };
                // Corpus size: union of all symbols seen across all three sources.
                let corpus_size = parity.symbols_registered as u32
                    + parity.symbols_documented as u32
                    + parity.symbols_used_in_corpus as u32;
                let mut report = AuditReport::complete(
                    gate_thing_name(),
                    binary_source_hash(&binary_source),
                    corpus_size,
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
                // Surface errors prominently; demote info-class counts to a
                // parenthetical suffix so they don't dominate the headline.
                // Audit doc §10 calls out that `documented_but_unused` and
                // `registered_but_undocumented` are completeness signals,
                // not drift errors.
                let parts: Vec<String> = parity
                    .summary()
                    .split_whitespace()
                    .map(str::to_string)
                    .collect();
                let (errors, infos): (Vec<String>, Vec<String>) =
                    parts.into_iter().partition(|s| {
                        s.starts_with("corpus_uses_unregistered=")
                            || s.starts_with("doc_claims_unregistered=")
                    });
                let mut note = format!("error_count={}", parity.error_count());
                if !errors.is_empty() {
                    note.push_str(&format!(" [{}]", errors.join(" ")));
                }
                if !infos.is_empty() {
                    note.push_str(&format!(" (info: {})", infos.join(" ")));
                }
                if !parity.is_clean() {
                    report.note = Some(format!("stdlib drift: {}", note));
                } else {
                    // Clean run — still surface info-class counts for visibility.
                    report.note = Some(format!("stdlib coverage clean ({})", note));
                }

                // Baseline comparison: if `--baseline` was supplied AND the
                // current error count is ≤ the baseline error count, treat
                // the run as passing (the gate is regression-only during
                // corpus cleanup per the audit doc §10 CI-wiring section).
                // Without a baseline, the gate fails on any error mismatch.
                let exit_code = match (parity.is_clean(), args.baseline.as_ref()) {
                    (true, _) => ExitCode::Ok,
                    (false, None) => ExitCode::BarMissed,
                    (false, Some(baseline_path)) => {
                        match read_baseline_error_count(baseline_path) {
                            Some(baseline_errors) => {
                                if parity.error_count() <= baseline_errors {
                                    // No regression — gate as Ok and annotate.
                                    let prior = report.note.clone().unwrap_or_default();
                                    report.note = Some(format!(
                                        "{prior} (baseline error_count={baseline_errors}; \
                                         no regression — gate passes)"
                                    ));
                                    ExitCode::Ok
                                } else {
                                    let prior = report.note.clone().unwrap_or_default();
                                    report.note = Some(format!(
                                        "{prior} (REGRESSION: baseline error_count={baseline_errors}, \
                                         current={})",
                                        parity.error_count(),
                                    ));
                                    ExitCode::BarMissed
                                }
                            }
                            None => ExitCode::BarMissed,
                        }
                    }
                };
                RunOutcome { report, exit_code }
            }
            Err(err) => RunOutcome {
                report: AuditReport::infra_error(
                    gate_thing_name(),
                    format!("stdlib-coverage check failed: {err}"),
                ),
                exit_code: ExitCode::InfrastructureError,
            },
        }
    }
}

fn gate_thing_name() -> &'static str {
    CrlGate::ToolingStdlibCoverage.thing_name()
}

/// Extract `error_count` from the `note` field of a prior canonical report.
/// Returns `None` if the file is missing, malformed, or the note doesn't
/// match the expected `(error_count=N)` pattern. A missing baseline is not
/// fatal — callers fall through to the absolute-comparison path.
fn read_baseline_error_count(path: &std::path::Path) -> Option<usize> {
    let json = std::fs::read_to_string(path).ok()?;
    let val: serde_json::Value = serde_json::from_str(&json).ok()?;
    let note = val.get("note")?.as_str()?;
    // Note shape: "stdlib drift: ... (error_count=41)"
    let key = "error_count=";
    let start = note.rfind(key)? + key.len();
    let tail = &note[start..];
    let end = tail.find(|c: char| !c.is_ascii_digit())?;
    tail[..end].parse::<usize>().ok()
}

/// Content hash of the binary source. The eval/builtins.rs file is the
/// authoritative source-of-truth; if it changes, the audit's view changes.
fn binary_source_hash(path: &std::path::Path) -> String {
    match std::fs::read(path) {
        Ok(bytes) => format!("blake3:{}", blake3::hash(&bytes).to_hex()),
        Err(_) => "blake3:unavailable".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stdlib_coverage_subcommand_dry_run_returns_ok() {
        let args = CommonArgs {
            dry_run: true,
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = StdlibCoverageSubcommand.run(&args);
        assert_eq!(outcome.exit_code, ExitCode::Ok);
        assert!(!outcome.report.incomplete);
    }

    #[test]
    fn stdlib_coverage_subcommand_runs_against_workspace() {
        let args = CommonArgs {
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = StdlibCoverageSubcommand.run(&args);
        // We don't gate on "no drift" because the audit is mid-cleanup —
        // some RegisteredButUndocumented warns are expected. We just verify
        // that the gate runs and produces a structurally complete report.
        assert!(!outcome.report.incomplete);
        assert_eq!(outcome.report.thing, "stdlib-coverage");
        assert!(
            outcome.report.corpus_size >= 30,
            "expected at least 30 unioned symbols, got {}",
            outcome.report.corpus_size,
        );
        assert!(
            outcome.report.corpus_hash.starts_with("blake3:"),
            "expected content hash; got {}",
            outcome.report.corpus_hash,
        );
    }

    #[test]
    fn stdlib_coverage_subcommand_with_missing_corpus_returns_infra_error_under_dry_run() {
        let args = CommonArgs {
            dry_run: true,
            corpus: Some(std::path::PathBuf::from("this/path/does/not/exist")),
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = StdlibCoverageSubcommand.run(&args);
        assert_eq!(outcome.exit_code, ExitCode::InvalidInput);
        assert!(outcome.report.incomplete);
    }

    /// When a baseline report is supplied and the current state has an
    /// equal-or-lower error count, the gate exits Ok with a "no regression"
    /// annotation. This is the CI gate's intended behavior during the
    /// Phase B/C corpus cleanup.
    #[test]
    fn stdlib_coverage_baseline_no_regression_exits_ok() {
        let workspace = workspace_root();
        let baseline = workspace
            .join("contracts/reports/stdlib-coverage/2026-05-23.json");
        if !baseline.exists() {
            // First run after the gate lands — skip rather than depend on a
            // freshly-checked-in baseline (which a separate run wrote).
            return;
        }
        let args = CommonArgs {
            baseline: Some(baseline),
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = StdlibCoverageSubcommand.run(&args);
        assert_eq!(
            outcome.exit_code,
            ExitCode::Ok,
            "expected baseline-compared run to pass; note: {:?}",
            outcome.report.note,
        );
        assert!(
            outcome
                .report
                .note
                .as_deref()
                .map(|n| n.contains("no regression"))
                .unwrap_or(false),
            "expected `no regression` annotation in note; got: {:?}",
            outcome.report.note,
        );
    }

    /// Helper used by tests above and any future regression assertion.
    fn workspace_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf()
    }

    #[test]
    fn stdlib_coverage_subcommand_threshold_defaults_to_one() {
        let args = CommonArgs {
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = StdlibCoverageSubcommand.run(&args);
        let threshold = outcome
            .report
            .threshold
            .expect("stdlib-coverage always emits a threshold");
        assert_eq!(threshold.target, 1.0);
    }
}
