//! A5 — full-typecheck gate over the golden corpus.
//!
//! `golden_vox_examples_test` only proves goldens *parse and lower*; it does NOT
//! run the typechecker. That gap let two flagship goldens (`crud_api.vox`,
//! `iot_telemetry.vox`) ship with `E0001` type errors while still being marked
//! `training_eligible`. This gate runs the same typecheck pass `vox check` uses
//! (`typecheck_module` → AST→HIR lower → `typecheck_hir_module`) over every
//! `examples/golden/**/*.vox` and fails on any Error-severity diagnostic.
//!
//! The `gate_actually_catches_type_errors` self-test guarantees this file can
//! never degrade into a false-green no-op.

use std::path::{Path, PathBuf};

use vox_compiler::lexer::lex;
use vox_compiler::parser::parse;
use vox_compiler::typeck::diagnostics::TypeckSeverity;
use vox_compiler::typeck::typecheck_module;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("../.."))
}

fn collect_golden_vox(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                collect_golden_vox(&p, out);
            } else if p.extension().is_some_and(|e| e == "vox") {
                out.push(p);
            }
        }
    }
}

/// Return Error-severity diagnostic messages for one `.vox` source, or `None`
/// when the source does not even parse (parse coverage is owned by
/// `golden_vox_examples_test`, so we skip rather than double-report).
fn typecheck_errors(src: &str, label: &str) -> Option<Vec<String>> {
    let tokens = lex(src);
    let module = match parse(tokens) {
        Ok(m) => m,
        Err(errs) => {
            eprintln!(
                "[golden_typecheck_gate] parse error in {label} (covered elsewhere): {errs:?}"
            );
            return None;
        }
    };
    let diags = typecheck_module(&module, src);
    Some(
        diags
            .into_iter()
            .filter(|d| d.severity == TypeckSeverity::Error)
            .map(|d| {
                format!(
                    "{}{}",
                    d.code
                        .as_deref()
                        .map(|c| format!("[{c}] "))
                        .unwrap_or_default(),
                    d.message
                )
            })
            .collect(),
    )
}

#[test]
fn all_golden_vox_examples_typecheck_clean() {
    let root = repo_root();
    let golden_dir = root.join("examples").join("golden");
    let mut files = Vec::new();
    collect_golden_vox(&golden_dir, &mut files);
    files.sort();
    assert!(
        !files.is_empty(),
        "no golden .vox files under {}",
        golden_dir.display()
    );

    let mut failures: Vec<(String, Vec<String>)> = Vec::new();
    for path in &files {
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[golden_typecheck_gate] IO error {}: {e}", path.display());
                continue;
            }
        };
        let label = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned();
        if let Some(errs) = typecheck_errors(&src, &label) {
            if !errs.is_empty() {
                failures.push((label, errs));
            }
        }
    }

    if failures.is_empty() {
        println!(
            "[golden_typecheck_gate] {} golden files typecheck clean ✓",
            files.len()
        );
    } else {
        let report: String = failures
            .iter()
            .map(|(label, errs)| {
                format!(
                    "  {label}\n{}",
                    errs.iter()
                        .map(|e| format!("      {e}\n"))
                        .collect::<String>()
                )
            })
            .collect();
        panic!(
            "{} golden file(s) fail `vox check` (typecheck):\n{}",
            failures.len(),
            report
        );
    }
}

/// Self-test: the gate must actually flag type errors. A `let` binding whose
/// declared type contradicts its initializer is a guaranteed Error-severity
/// diagnostic; if this ever returns zero errors, the gate above is a no-op and
/// this test fails loudly.
#[test]
fn gate_actually_catches_type_errors() {
    let bad = "fn broken() to int {\n  let x: int = \"not an int\"\n  return x\n}\n";
    let errs = typecheck_errors(bad, "<self-test>").expect("self-test source must parse");
    assert!(
        !errs.is_empty(),
        "typecheck gate failed to flag an obvious type error — the gate is a no-op"
    );
}
