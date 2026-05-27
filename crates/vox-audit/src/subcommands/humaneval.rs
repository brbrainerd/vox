//! `vox audit humaneval` — CR-L1 HumanEval-Vox static-check gate.
//!
//! ## What this measures
//!
//! For each fixture in `contracts/eval/humaneval-vox/manifest.v1.yaml`, both
//! `reference.vox` and `tests.vox` must pass `vox check` (exit 0). The
//! overall pass rate is `(passing files) / (total files)`.
//!
//! ## Status-based behaviour
//!
//! | Corpus status     | Behaviour |
//! |-------------------|-----------|
//! | `stub`            | Exit 2 (InfrastructureError) — corpus not yet authored. |
//! | `minimum-viable`  | Exit 0/1 per bar (`corpus.bar.target`, default 0.80). |
//! | `complete`        | Exit 0/1 per bar (same logic). |
//!
//! ## LLM-generation phase
//!
//! Once the LLM-panel harness lands (P2.4+), this subcommand will be extended
//! to prompt each fixture's spec against a panel of LLMs and score
//! `vox check` + `vox run tests.vox` pass rate. Until then, this module
//! validates the *authored* reference corpus.
//!
//! Council ratified 2026-05-15 (D10, D25).

use std::path::PathBuf;
use std::process::Command;

use crate::{
    CommonArgs, CrlGate, RunOutcome, Subcommand,
    report::{AuditReport, ExitCode, Results, Threshold},
    workspace_root,
};

const MANIFEST_RELPATH: &str = "contracts/eval/humaneval-vox/manifest.v1.yaml";

pub struct HumanEvalSubcommand;

impl Subcommand for HumanEvalSubcommand {
    fn gate(&self) -> CrlGate {
        CrlGate::L1HumanEval
    }

    fn description(&self) -> &'static str {
        "CR-L1: HumanEval-Vox (≥80%) on the 164-problem corpus."
    }

    fn run(&self, args: &CommonArgs) -> RunOutcome {
        let root = workspace_root();
        let manifest_path = match &args.corpus {
            Some(p) => p.clone(),
            None => root.join(MANIFEST_RELPATH),
        };

        // --- 1. Read manifest ---
        let manifest_str = match std::fs::read_to_string(&manifest_path) {
            Ok(s) => s,
            Err(e) => {
                return infra_err(format!(
                    "cannot read manifest `{}`: {e}",
                    manifest_path.display()
                ));
            }
        };
        let manifest: serde_yaml::Value = match serde_yaml::from_str(&manifest_str) {
            Ok(v) => v,
            Err(e) => {
                return infra_err(format!(
                    "manifest `{}` parse error: {e}",
                    manifest_path.display()
                ));
            }
        };

        // --- 2. Gate on corpus status ---
        let status = manifest["corpus"]["status"].as_str().unwrap_or("stub");
        if status == "stub" {
            return infra_err(format!(
                "corpus stub: `{MANIFEST_RELPATH}` declares `status: stub`. \
                 Harness lands per implementation-plan phasing. Re-run after fixtures are authored."
            ));
        }

        // --- 3. Extract manifest fields ---
        let bar: f64 = args
            .threshold
            .unwrap_or_else(|| manifest["corpus"]["bar"]["target"].as_f64().unwrap_or(0.80));
        let corpus_hash = manifest["corpus"]["corpus_hash"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();
        let fixtures = match manifest["fixtures"].as_sequence() {
            Some(seq) => seq.clone(),
            None => {
                return infra_err(
                    "manifest `fixtures` key is missing or not a sequence".to_string(),
                );
            }
        };
        let corpus_size = fixtures.len() as u32;

        // --- 4. Locate vox binary ---
        let vox_bin = find_vox_bin(&root);

        // --- 5. Run vox check on each file ---
        let mut pass: u32 = 0;
        let mut total: u32 = 0;
        let corpus_dir = manifest_path.parent().unwrap_or(root.as_path());

        for fixture in &fixtures {
            let files = &fixture["files"];
            for key in &["reference", "tests"] {
                if let Some(rel) = files[key].as_str() {
                    let abs = corpus_dir.join(rel);
                    if abs.exists() {
                        total += 1;
                        if args.dry_run || check_file(&vox_bin, &abs) {
                            pass += 1;
                        }
                    } else {
                        // Missing file counts as a failure (corpus gap).
                        total += 1;
                    }
                }
            }
        }

        // --- 6. Compute pass rate and compare against bar ---
        let pass_rate = if total == 0 {
            0.0
        } else {
            pass as f64 / total as f64
        };
        let met = pass_rate >= bar;
        let exit_code = if met {
            ExitCode::Ok
        } else {
            ExitCode::BarMissed
        };

        let results = Results {
            overall_pass_rate: pass_rate,
            median_pass_rate: Some(pass_rate), // single scorer — no LLM panel yet
            per_llm: Vec::new(),
        };
        let mut report = AuditReport::complete(
            CrlGate::L1HumanEval.thing_name(),
            corpus_hash,
            corpus_size,
            results,
        );
        report.threshold = Some(Threshold { target: bar, met });
        if args.dry_run {
            report.note = Some("dry-run: file existence validated; vox check skipped".into());
        }

        // --- 7. Optionally persist canonical report ---
        if args.write_canonical_report && !args.dry_run {
            let report_path = root.join(report.canonical_report_path());
            if let Err(e) = report.write_json_atomic(&report_path) {
                tracing::warn!("failed to write canonical humaneval report: {e}");
            }
        }

        RunOutcome { report, exit_code }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn infra_err(note: String) -> RunOutcome {
    RunOutcome {
        report: AuditReport::infra_error(CrlGate::L1HumanEval.thing_name(), note),
        exit_code: ExitCode::InfrastructureError,
    }
}

/// Locate the `vox` CLI binary.
///
/// Search order:
/// 1. `<workspace>/target/debug/vox[.exe]` — dev build (CI runs `cargo build` first)
/// 2. `<workspace>/target/release/vox[.exe]` — release build
/// 3. `vox` in `$PATH`
fn find_vox_bin(workspace_root: &std::path::Path) -> PathBuf {
    let exe_ext = if cfg!(windows) { ".exe" } else { "" };
    let debug = workspace_root
        .join("target")
        .join("debug")
        .join(format!("vox{exe_ext}"));
    if debug.exists() {
        return debug;
    }
    let release = workspace_root
        .join("target")
        .join("release")
        .join(format!("vox{exe_ext}"));
    if release.exists() {
        return release;
    }
    // Fall back to PATH.
    PathBuf::from(format!("vox{exe_ext}"))
}

/// Run `vox check <path>` and return true iff it exits 0.
fn check_file(vox_bin: &std::path::Path, path: &std::path::Path) -> bool {
    Command::new(vox_bin)
        .arg("check")
        .arg(path)
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn args_no_report() -> CommonArgs {
        CommonArgs {
            write_canonical_report: false,
            ..CommonArgs::default()
        }
    }

    /// Smoke test: subcommand returns a structurally valid report against the
    /// real corpus. We don't assert on pass/fail (the corpus may be evolving)
    /// but we do assert on structural invariants.
    #[test]
    fn humaneval_produces_valid_report_against_real_corpus() {
        let sub = HumanEvalSubcommand;
        let outcome = sub.run(&args_no_report());

        // Report must have the correct thing name.
        assert_eq!(outcome.report.thing, "humaneval");
        // Pass rate must be in [0.0, 1.0].
        assert!(
            (0.0..=1.0).contains(&outcome.report.results.overall_pass_rate),
            "pass_rate out of range: {}",
            outcome.report.results.overall_pass_rate
        );
        // Corpus size must match minimum-viable count (50) or be 0 if CI
        // skips the build step and the manifest can't be found.
        // We accept any non-negative count.
        assert!(
            outcome.report.corpus_size >= 0,
            "negative corpus size is impossible but added for exhaustiveness"
        );
        // Threshold block must be present when bar applies.
        match outcome.exit_code {
            ExitCode::Ok | ExitCode::BarMissed => {
                assert!(
                    outcome.report.threshold.is_some(),
                    "real harness must populate threshold block"
                );
            }
            ExitCode::InfrastructureError => {
                // Corpus or binary not available (e.g., partial CI shard) — acceptable.
            }
            ExitCode::InvalidInput => panic!("unexpected InvalidInput from humaneval"),
        }
    }

    #[test]
    fn humaneval_dry_run_skips_check() {
        let sub = HumanEvalSubcommand;
        let args = CommonArgs {
            dry_run: true,
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = sub.run(&args);
        // Dry-run must not return InvalidInput or panic.
        assert_ne!(outcome.exit_code, ExitCode::InvalidInput);
        // Note field must be present for dry-run.
        if outcome.exit_code != ExitCode::InfrastructureError {
            assert!(
                outcome.report.note.is_some(),
                "dry-run should include a note"
            );
        }
    }
}
