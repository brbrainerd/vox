//! Triage helper: reports which `examples/golden/` fixtures successfully lower to
//! TypeScript, so they can be promoted into `examples/golden-ts/` (the typecheck gate corpus).
//!
//! This is a REPORTING test — it never fails on a fixture that cannot emit. It prints
//! two lists so a human/agent can promote the emitting ones.
//!
//! Run: cargo nextest run -p vox-integration-tests --test ts_emit_corpus_triage_test --run-ignored ignored-only --no-capture

#![allow(missing_docs)]
#![allow(unsafe_code)]

use std::path::{Path, PathBuf};

use vox_codegen::codegen_ts::emitter::BuildMode;
use vox_codegen::codegen_ts::{CodegenOptions, generate_with_options};
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/golden")
}

fn collect_vox_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().is_some_and(|e| e == "vox") {
                files.push(p);
            }
        }
    }
    files.sort();
    files
}

/// Returns Ok(count_of_ts_files) if the fixture lowers to TS, Err(reason) otherwise.
fn try_emit(src: &str) -> Result<usize, String> {
    let tokens = lex(src);
    let module = parse(tokens).map_err(|e| format!("parse: {e:?}"))?;
    let hir = lower_module(&module);
    let opts = CodegenOptions {
        tanstack_start: false,
        target: None,
        mode: BuildMode::App,
        ..Default::default()
    };
    unsafe { std::env::set_var("VOX_WEBIR_VALIDATE", "0") };
    let result = generate_with_options(&hir, opts);
    unsafe { std::env::remove_var("VOX_WEBIR_VALIDATE") };

    let output = result.map_err(|e| format!("codegen: {e}"))?;
    let ts_count = output
        .files
        .iter()
        .filter(|(n, _)| n.ends_with(".ts") || n.ends_with(".tsx"))
        .count();
    if ts_count == 0 {
        return Err("emitted no .ts/.tsx files".to_string());
    }
    Ok(ts_count)
}

#[test]
#[ignore = "reporting-only triage; run with --run-ignored ignored-only — owner: integration-tests sunset: 2026-12-31"]
fn report_golden_fixtures_that_emit_typescript() {
    let files = collect_vox_files(&golden_dir());
    assert!(!files.is_empty(), "No .vox files found in examples/golden/");

    let mut emitting = Vec::new();
    let mut skipped = Vec::new();

    for path in &files {
        let label = path.file_stem().unwrap().to_string_lossy().to_string();
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                skipped.push((label, format!("read: {e}")));
                continue;
            }
        };
        // Emission can panic on unsupported constructs; contain it so triage completes.
        // std::panic::catch_unwind loses the payload's Display text by default, so
        // extract it explicitly rather than reporting a bare "panicked".
        match std::panic::catch_unwind(|| try_emit(&src)) {
            Ok(Ok(n)) => emitting.push((label, n)),
            Ok(Err(reason)) => skipped.push((label, reason)),
            Err(payload) => {
                let msg = payload
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| payload.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "non-string panic payload".to_string());
                skipped.push((label, format!("panicked during emit: {msg}")));
            }
        }
    }

    println!(
        "\n=== PROMOTABLE ({} fixtures emit TypeScript) ===",
        emitting.len()
    );
    for (label, n) in &emitting {
        println!("  {label}  ({n} ts/tsx files)");
    }
    println!("\n=== NOT PROMOTABLE ({}) ===", skipped.len());
    for (label, reason) in &skipped {
        println!("  {label}: {reason}");
    }
    println!("\nPromote with: cp examples/golden/<name>.vox examples/golden-ts/<name>.vox\n");
}
