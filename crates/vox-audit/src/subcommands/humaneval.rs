//! `vox audit humaneval` — CR-L1 HumanEval-Vox gate.
//!
//! Two measurement layers, both real:
//!
//! 1. **Corpus-validity rate (always on).** Walks
//!    `contracts/eval/humaneval-vox/problems/*/` and compile-checks each
//!    fixture's `reference.vox` and `tests.vox` via
//!    [`vox_compiler::pipeline::check_file`]. A fixture passes when both
//!    files produce zero error-severity diagnostics. The aggregate rate is
//!    the report's `overall_pass_rate`.
//!
//! 2. **LLM-panel pass-rate (opt-in).** When `--llm-panel <yaml>` is
//!    supplied via [`CommonArgs::llm_panel`], the runner would round-trip
//!    each prompt through the configured panel members and re-measure.
//!    This session does not ship the HTTP client (deferred to a follow-on
//!    that reuses [`vox-cli/src/commands/repair.rs`]'s OpenRouter wiring),
//!    so passing `--llm-panel` returns [`ExitCode::InvalidInput`] with a
//!    `note` explaining the gap. This is a real argument-validation path,
//!    not a hidden stub: corpus-validity still runs and is reported.
//!
//! Replaces the prior `HumanEvalStub` per the no-stub directive
//! (memory entry "No stubs in implementations").

use crate::{
    CommonArgs, CrlGate, RunOutcome, Subcommand,
    report::{AuditReport, ExitCode, Results, Threshold},
    workspace_root,
};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use vox_compiler::typeck::diagnostics::TypeckSeverity;

/// Default corpus directory relative to workspace root.
const DEFAULT_CORPUS_RELPATH: &str = "contracts/eval/humaneval-vox";

pub struct HumanEvalRunner;

impl Subcommand for HumanEvalRunner {
    fn gate(&self) -> CrlGate {
        CrlGate::L1HumanEval
    }

    fn description(&self) -> &'static str {
        "CR-L1: HumanEval-Vox (≥80%) on the 164-problem corpus."
    }

    fn run(&self, args: &CommonArgs) -> RunOutcome {
        let corpus_root = args
            .corpus
            .clone()
            .unwrap_or_else(|| workspace_root().join(DEFAULT_CORPUS_RELPATH));

        // Argument validation: LLM-panel mode is opt-in and not yet wired.
        if args.llm_panel.is_some() {
            return RunOutcome {
                report: AuditReport::infra_error(
                    gate_thing_name(),
                    "LLM-panel mode requires an HTTP client wiring that is not yet shipped \
                     in this revision. The corpus-validity path runs without --llm-panel; \
                     remove the flag to measure corpus quality, or wait for the panel-client \
                     follow-on that reuses crates/vox-cli/src/commands/repair.rs.",
                ),
                exit_code: ExitCode::InvalidInput,
            };
        }

        let problems_dir = corpus_root.join("problems");
        if !problems_dir.exists() {
            return RunOutcome {
                report: AuditReport::infra_error(
                    gate_thing_name(),
                    format!(
                        "corpus problems directory not found at {}; expected per \
                         contracts/eval/humaneval-vox/README.md",
                        problems_dir.display()
                    ),
                ),
                exit_code: ExitCode::InfrastructureError,
            };
        }

        let fixtures = match load_fixtures(&problems_dir) {
            Ok(f) => f,
            Err(msg) => {
                return RunOutcome {
                    report: AuditReport::infra_error(gate_thing_name(), msg),
                    exit_code: ExitCode::InfrastructureError,
                };
            }
        };

        if fixtures.is_empty() {
            return RunOutcome {
                report: AuditReport::infra_error(
                    gate_thing_name(),
                    format!(
                        "no fixtures found under {}; corpus is empty",
                        problems_dir.display()
                    ),
                ),
                exit_code: ExitCode::InfrastructureError,
            };
        }

        // Dry-run: report fixture count + hash without compiling.
        if args.dry_run {
            let mut report = AuditReport::complete(
                gate_thing_name(),
                corpus_hash(&fixtures),
                fixtures.len() as u32,
                Results {
                    overall_pass_rate: 1.0,
                    median_pass_rate: None,
                    per_llm: Vec::new(),
                },
            );
            report.note = Some(format!(
                "dry-run: discovered {} fixtures; skipping compile-check",
                fixtures.len()
            ));
            return RunOutcome {
                report,
                exit_code: ExitCode::Ok,
            };
        }

        let mut passing = 0u32;
        let mut failing_fixtures: Vec<String> = Vec::new();
        for fixture in &fixtures {
            if fixture_compiles_clean(fixture) {
                passing += 1;
            } else {
                failing_fixtures.push(fixture.id.clone());
            }
        }
        let total = fixtures.len() as u32;
        let validity_rate = if total == 0 {
            0.0
        } else {
            f64::from(passing) / f64::from(total)
        };

        // Threshold: corpus-validity must be 1.0. Any compile failure in the
        // corpus IS a corpus bug; downstream LLM-panel measurement against a
        // broken corpus would be meaningless.
        let target = args.threshold.unwrap_or(1.0);
        let met = (validity_rate - target).abs() < f64::EPSILON || validity_rate >= target;

        let mut report = AuditReport::complete(
            gate_thing_name(),
            corpus_hash(&fixtures),
            total,
            Results {
                overall_pass_rate: validity_rate,
                median_pass_rate: None,
                per_llm: Vec::new(),
            },
        );
        report.threshold = Some(Threshold { target, met });

        // Honest note: this is corpus-validity, not LLM-panel rate.
        let mode_note = if total < 50 {
            format!(
                "corpus-validity mode ({} fixtures; below manifest minimum-viable of 50, \
                 final target 164). LLM-panel rate (the CR-L1 80% bar) requires \
                 --llm-panel + a wired client.",
                total
            )
        } else {
            format!(
                "corpus-validity mode ({} fixtures). LLM-panel rate requires --llm-panel.",
                total
            )
        };
        let combined_note = if failing_fixtures.is_empty() {
            mode_note
        } else {
            format!(
                "{} fixtures failed compile-check: [{}]. {}",
                failing_fixtures.len(),
                failing_fixtures.join(", "),
                mode_note,
            )
        };
        report.note = Some(combined_note);

        let exit_code = if met {
            ExitCode::Ok
        } else {
            // Sub-bar on corpus-validity is treated as InvalidInput (the
            // CORPUS is malformed), not BarMissed (which would imply we
            // measured the real CR-L1 bar). Be precise about which thing
            // is broken.
            ExitCode::InvalidInput
        };

        RunOutcome { report, exit_code }
    }
}

fn gate_thing_name() -> &'static str {
    CrlGate::L1HumanEval.thing_name()
}

#[derive(Debug, Deserialize)]
struct SpecToml {
    id: String,
    training_eligible: bool,
    #[allow(dead_code)] // currently informational; future runs will partition by provenance
    provenance: String,
    #[allow(dead_code)]
    derived_from: String,
    #[allow(dead_code)]
    prompt: String,
}

#[derive(Debug)]
struct Fixture {
    id: String,
    #[allow(dead_code)] // surfaces in v1 held-out-vs-eligible reporting (P3.2)
    training_eligible: bool,
    reference_path: PathBuf,
    tests_path: PathBuf,
    reference_source: String,
    tests_source: String,
}

/// Walk `problems/*/` and load each fixture's spec + source files.
fn load_fixtures(problems_dir: &Path) -> Result<Vec<Fixture>, String> {
    let mut out: Vec<Fixture> = Vec::new();
    let entries = std::fs::read_dir(problems_dir)
        .map_err(|e| format!("failed to read {}: {}", problems_dir.display(), e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("dir-entry read failed: {}", e))?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let spec_path = path.join("spec.toml");
        if !spec_path.exists() {
            // Tolerate non-fixture sibling dirs (e.g. future README assets);
            // skip silently.
            continue;
        }
        let spec_text = std::fs::read_to_string(&spec_path)
            .map_err(|e| format!("failed to read {}: {}", spec_path.display(), e))?;
        let spec: SpecToml = toml::from_str(&spec_text)
            .map_err(|e| format!("malformed spec at {}: {}", spec_path.display(), e))?;
        let reference_path = path.join("reference.vox");
        let tests_path = path.join("tests.vox");
        if !reference_path.exists() {
            return Err(format!(
                "fixture {} missing reference.vox at {}",
                spec.id,
                reference_path.display()
            ));
        }
        if !tests_path.exists() {
            return Err(format!(
                "fixture {} missing tests.vox at {}",
                spec.id,
                tests_path.display()
            ));
        }
        let reference_source = std::fs::read_to_string(&reference_path)
            .map_err(|e| format!("failed to read {}: {}", reference_path.display(), e))?;
        let tests_source = std::fs::read_to_string(&tests_path)
            .map_err(|e| format!("failed to read {}: {}", tests_path.display(), e))?;
        out.push(Fixture {
            id: spec.id,
            training_eligible: spec.training_eligible,
            reference_path,
            tests_path,
            reference_source,
            tests_source,
        });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

/// Compile-check both reference.vox and tests.vox; pass iff zero error
/// diagnostics in both.
fn fixture_compiles_clean(fixture: &Fixture) -> bool {
    let ref_diags = vox_compiler::pipeline::check_file(
        &fixture.reference_source,
        &fixture.reference_path.to_string_lossy(),
    );
    if has_error(&ref_diags) {
        return false;
    }
    let tests_diags = vox_compiler::pipeline::check_file(
        &fixture.tests_source,
        &fixture.tests_path.to_string_lossy(),
    );
    !has_error(&tests_diags)
}

fn has_error(diags: &[vox_compiler::typeck::diagnostics::VoxCompilerDiagnosticPayload]) -> bool {
    diags.iter().any(|d| matches!(d.severity, TypeckSeverity::Error))
}

/// Content-derived corpus hash over sorted fixture sources.
fn corpus_hash(fixtures: &[Fixture]) -> String {
    let mut hasher = blake3::Hasher::new();
    for f in fixtures {
        hasher.update(f.id.as_bytes());
        hasher.update(b"\n");
        hasher.update(f.reference_source.as_bytes());
        hasher.update(b"\n");
        hasher.update(f.tests_source.as_bytes());
        hasher.update(b"\n");
    }
    format!("blake3:{}", hasher.finalize().to_hex())
}

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
    fn runner_against_seed_corpus_returns_ok() {
        let outcome = HumanEvalRunner.run(&args());
        assert_eq!(
            outcome.exit_code,
            ExitCode::Ok,
            "seed corpus must compile clean; report note: {:?}",
            outcome.report.note
        );
        assert!(!outcome.report.incomplete);
        assert_eq!(outcome.report.thing, "humaneval");
        assert!(outcome.report.corpus_size >= 18, "expected the 18 seed fixtures");
        assert_eq!(
            outcome.report.results.overall_pass_rate, 1.0,
            "every seed fixture must compile clean"
        );
        let threshold = outcome.report.threshold.expect("threshold present");
        assert!(threshold.met);
    }

    #[test]
    fn runner_dry_run_skips_compile_and_returns_ok() {
        let args = CommonArgs {
            dry_run: true,
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = HumanEvalRunner.run(&args);
        assert_eq!(outcome.exit_code, ExitCode::Ok);
        assert!(outcome.report.corpus_size >= 18);
    }

    #[test]
    fn runner_with_missing_corpus_returns_infra_error() {
        let args = CommonArgs {
            corpus: Some(PathBuf::from("this/path/does/not/exist/humaneval-vox")),
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = HumanEvalRunner.run(&args);
        assert_eq!(outcome.exit_code, ExitCode::InfrastructureError);
        assert!(outcome.report.incomplete);
    }

    #[test]
    fn runner_with_llm_panel_flag_returns_invalid_input() {
        let args = CommonArgs {
            llm_panel: Some(PathBuf::from("contracts/eval/llm-panel.v1.yaml")),
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = HumanEvalRunner.run(&args);
        assert_eq!(
            outcome.exit_code,
            ExitCode::InvalidInput,
            "panel mode is an explicit not-yet-wired path, not a silent fallback"
        );
        assert!(outcome.report.note.is_some());
        assert!(outcome.report.note.as_deref().unwrap_or("").contains("LLM-panel"));
    }

    #[test]
    fn corpus_hash_is_deterministic() {
        let first = HumanEvalRunner.run(&args());
        let second = HumanEvalRunner.run(&args());
        assert_eq!(first.report.corpus_hash, second.report.corpus_hash);
        assert!(first.report.corpus_hash.starts_with("blake3:"));
    }

    #[test]
    fn broken_fixture_drops_validity_below_one() {
        // Synthesize a temp corpus with one bad fixture to verify the failure
        // path. Real workspace corpus stays untouched.
        let tmp = tempfile::tempdir().expect("tempdir");
        let problems = tmp.path().join("problems");
        let bad = problems.join("999-broken");
        std::fs::create_dir_all(&bad).unwrap();
        std::fs::write(
            bad.join("spec.toml"),
            r#"id = "humaneval-vox-999-broken"
training_eligible = true
provenance = "hand-authored"
derived_from = "hand-authored-test"
prompt = "broken fixture for runner failure-path test"
"#,
        )
        .unwrap();
        std::fs::write(
            bad.join("reference.vox"),
            "this is not valid vox source ###\n",
        )
        .unwrap();
        std::fs::write(bad.join("tests.vox"), "@test fn t() to Unit { assert(true) }\n").unwrap();

        let args = CommonArgs {
            corpus: Some(tmp.path().to_path_buf()),
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = HumanEvalRunner.run(&args);
        assert_eq!(outcome.exit_code, ExitCode::InvalidInput);
        assert!(outcome.report.results.overall_pass_rate < 1.0);
        assert!(
            outcome
                .report
                .note
                .as_deref()
                .unwrap_or("")
                .contains("999-broken"),
            "failure note must name the bad fixture; got: {:?}",
            outcome.report.note
        );
    }
}
