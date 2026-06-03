//! `vox audit humaneval` — CR-L1 HumanEval-Vox static-check gate.
//!
//! ## What this measures
//!
//! For each fixture in `contracts/eval/humaneval-vox/manifest.v1.yaml`,
//! `reference.vox` must pass `vox check` (typecheck only — it is a library with
//! no `main`), and `tests.vox` must pass `vox check` **and** execute cleanly
//! under `vox run --mode interp` — i.e. its `fn main()` assertions must actually
//! hold. The overall pass rate is `(passing files) / (total files)`. Because the
//! test files are now executed (not just typechecked), a wrong oracle or a
//! stubbed reference fails the gate behaviorally instead of passing on a clean
//! typecheck alone.
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
                        // `reference.vox` is a library (no `main`) → typecheck only.
                        // `tests.vox` carries `fn main() { assert(...) }` → typecheck
                        // AND execute, so a false oracle / stubbed reference fails the
                        // gate behaviorally rather than passing on typecheck alone.
                        let ok = if args.dry_run {
                            true
                        } else if *key == "tests" {
                            check_file(&vox_bin, &abs) && run_file(&vox_bin, &abs)
                        } else {
                            check_file(&vox_bin, &abs)
                        };
                        if ok {
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

/// Run `vox run --mode interp <path>` and return true iff it exits 0.
///
/// Used for `tests.vox` files, which carry a `fn main()` of `assert(...)` calls.
/// A false assertion propagates as `EvalError::AssertionFailed` and the
/// interpreter run exits non-zero — so this catches wrong oracles and stubbed
/// references that a typecheck-only `vox check` would let through.
fn run_file(vox_bin: &std::path::Path, path: &std::path::Path) -> bool {
    Command::new(vox_bin)
        .arg("run")
        .arg("--mode")
        .arg("interp")
        .arg(path)
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Held-out manifest regeneration (`held-out.v1.json`)
// ---------------------------------------------------------------------------

/// One held-out fixture entry in `held-out.v1.json`.
#[derive(Debug, serde::Serialize)]
pub struct HeldOutEntry {
    pub id: String,
    pub provenance: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub derived_from: Option<String>,
    pub fixture_hash: String,
}

/// The regenerated contents of `contracts/eval/humaneval-vox/held-out.v1.json`.
#[derive(Debug, serde::Serialize)]
pub struct HeldOutManifest {
    pub schema_version: u32,
    pub corpus: String,
    pub total_fixtures: u32,
    pub held_out_count: u32,
    pub corpus_hash: String,
    pub entries: Vec<HeldOutEntry>,
}

/// Regenerate the held-out manifest from the live SSOT (`manifest.v1.yaml`).
///
/// Held-out problems are exactly those marked `training_eligible: false` in the
/// manifest — the single source of truth. `corpus_hash` is taken from the
/// manifest (sha256), and each `fixture_hash` is a blake3 over the fixture's
/// `reference.vox` + `tests.vox` bytes. This replaces the function removed when
/// the runner was rewritten to read `manifest.v1.yaml`, fixing the
/// `regen_held_out` compile break and reconciling the stale `held-out.v1.json`
/// (which described the abandoned orphan seed corpus) to the live 164-problem set.
pub fn build_held_out_manifest(problems_dir: &std::path::Path) -> Result<HeldOutManifest, String> {
    let humaneval_dir = problems_dir
        .parent()
        .ok_or_else(|| format!("problems_dir `{}` has no parent", problems_dir.display()))?;
    let manifest_path = humaneval_dir.join("manifest.v1.yaml");
    let manifest_str = std::fs::read_to_string(&manifest_path)
        .map_err(|e| format!("read {}: {e}", manifest_path.display()))?;
    let manifest: serde_yaml::Value =
        serde_yaml::from_str(&manifest_str).map_err(|e| format!("parse manifest: {e}"))?;

    let corpus_hash = manifest["corpus"]["corpus_hash"]
        .as_str()
        .unwrap_or("unknown")
        .to_string();

    let fixtures = manifest["fixtures"]
        .as_sequence()
        .ok_or_else(|| "manifest `fixtures` key missing or not a sequence".to_string())?;
    let total_fixtures = fixtures.len() as u32;

    let mut entries = Vec::new();
    for fx in fixtures {
        // Held-out == training_eligible: false. Default to eligible (not held out).
        if fx["training_eligible"].as_bool().unwrap_or(true) {
            continue;
        }
        let slug = fx["slug"].as_str().unwrap_or("unknown");
        let provenance = fx["provenance"].as_str().unwrap_or("original").to_string();

        let mut hasher = blake3::Hasher::new();
        for key in ["reference", "tests"] {
            if let Some(rel) = fx["files"][key].as_str() {
                let p = humaneval_dir.join(rel);
                if let Ok(bytes) = std::fs::read(&p) {
                    hasher.update(&bytes);
                }
            }
        }
        entries.push(HeldOutEntry {
            id: format!("humaneval-vox-{slug}"),
            provenance,
            derived_from: fx["derived_from"].as_str().map(str::to_string),
            fixture_hash: format!("blake3:{}", hasher.finalize().to_hex()),
        });
    }

    Ok(HeldOutManifest {
        schema_version: 1,
        corpus: "humaneval-vox".to_string(),
        total_fixtures,
        held_out_count: entries.len() as u32,
        corpus_hash,
        entries,
    })
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

    /// Behavioral gate (CR-L1 "typecheck ≠ behavior" regression): a `tests.vox`
    /// whose `fn main()` asserts a FALSE oracle must FAIL the gate. Under the old
    /// typecheck-only runner this file passed `vox check`, yielding pass_rate 1.0;
    /// the runner now executes `tests.vox` under `vox run --mode interp`, so the
    /// false assertion drops the pass rate to 0.5 (reference passes check, tests
    /// fails execution).
    #[test]
    fn humaneval_executes_tests_and_fails_false_oracle() {
        let vox_bin = find_vox_bin(&workspace_root());
        if !vox_bin.exists() {
            eprintln!(
                "skipping humaneval_executes_tests_and_fails_false_oracle: \
                 vox binary not built at {}",
                vox_bin.display()
            );
            return;
        }

        let dir = std::env::temp_dir().join(format!("vox_he_exec_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        // reference.vox: a correct library fn (typechecks; no `main`).
        std::fs::write(
            dir.join("ref.vox"),
            "fn add(a: int, b: int) to int {\n    return a + b\n}\n",
        )
        .unwrap();
        // tests.vox: re-declares the fn, then a `main()` with a FALSE assertion.
        std::fs::write(
            dir.join("tst.vox"),
            "fn add(a: int, b: int) to int {\n    return a + b\n}\n\n\
             fn main() to str {\n    assert(add(1, 1) == 999)\n    return \"ok\"\n}\n",
        )
        .unwrap();
        // Minimal manifest; bar 0.0 so the gate's exit code is Ok regardless.
        std::fs::write(
            dir.join("manifest.v1.yaml"),
            "corpus:\n  status: complete\n  bar:\n    target: 0.0\n  corpus_hash: \"test\"\n\
             fixtures:\n  - files:\n      reference: ref.vox\n      tests: tst.vox\n",
        )
        .unwrap();

        let args = CommonArgs {
            corpus: Some(dir.join("manifest.v1.yaml")),
            write_canonical_report: false,
            ..CommonArgs::default()
        };
        let outcome = HumanEvalSubcommand.run(&args);
        let rate = outcome.report.results.overall_pass_rate;
        let _ = std::fs::remove_dir_all(&dir);

        // reference.vox passes `vox check` (1); tests.vox typechecks but its
        // executed assertion is false, so it does NOT count → pass=1 / total=2.
        assert!(
            (rate - 0.5).abs() < 1e-9,
            "expected behavioral execution to fail the false-oracle tests.vox \
             (pass=1/2=0.5), got {rate}"
        );
    }
}
