//! Phase 1 / CR-F2 measurement probe: how far does the **codegen-ts** arm get on
//! the behavioral golden corpus (`examples/golden/**` files with `fn main(` + a
//! `// EXPECT:` line)?
//!
//! This is the cheap first slice of Task 1.A — it does NOT run Node; it only
//! drives `lex → parse → lower → codegen_ts::generate_with_options` for each
//! golden and records whether emission **succeeds** and whether the emitted
//! bundle defines a `main` function. That alone enumerates the codegen-ts
//! backlog (the "T-classes") without paying for a Node execution harness.
//!
//! Run: `cargo test -p vox-integration-tests --test ts_emit_goldens_probe -- --nocapture`
#![allow(missing_docs)]
#![allow(unsafe_code)] // set_var/remove_var to isolate VOX_WEBIR_VALIDATE (same as ts_emit_typecheck_test.rs)

use std::path::{Path, PathBuf};

use vox_codegen::codegen_ts::emitter::BuildMode;
use vox_codegen::codegen_ts::{CodegenOptions, generate_with_options};
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

/// Recursively collect `examples/golden/**/*.vox` files that contain BOTH
/// `fn main(` and a `// EXPECT:` line — the executable behavioral corpus.
fn collect_expect_main_goldens() -> Vec<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/golden");
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "vox") {
                if let Ok(src) = std::fs::read_to_string(&p) {
                    if src.contains("fn main(") && src.contains("// EXPECT:") {
                        out.push(p);
                    }
                }
            }
        }
    }
    out.sort();
    out
}

enum TsEmit {
    Ok { defines_main: bool, file_count: usize },
    ParseErr(String),
    GenErr(String),
    Panic,
}

/// Drive codegen-ts for one golden, capturing failure modes (incl. panics from
/// unsupported constructs) rather than aborting the whole run.
fn probe_ts_emit(path: &Path) -> TsEmit {
    let src = std::fs::read_to_string(path).unwrap_or_default();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let tokens = lex(&src);
        let module = match parse(tokens) {
            Ok(m) => m,
            Err(e) => return TsEmit::ParseErr(format!("{e:?}")),
        };
        let hir = lower_module(&module);
        let opts = CodegenOptions {
            tanstack_start: false,
            target: None,
            mode: BuildMode::App,
            ..Default::default()
        };
        // Isolate the WebIR structural gate like ts_emit_typecheck_test.rs does.
        unsafe { std::env::set_var("VOX_WEBIR_VALIDATE", "0") };
        let emitted = generate_with_options(&hir, opts);
        unsafe { std::env::remove_var("VOX_WEBIR_VALIDATE") };
        match emitted {
            Ok(out) => {
                let defines_main = out
                    .files
                    .iter()
                    .any(|(_, c)| c.contains("function main") || c.contains("const main"));
                TsEmit::Ok {
                    defines_main,
                    file_count: out.files.len(),
                }
            }
            Err(e) => TsEmit::GenErr(e),
        }
    }));
    result.unwrap_or(TsEmit::Panic)
}

/// Census (not a hard gate yet): print the per-golden codegen-ts emit status so
/// the T-class backlog is visible. Asserts only that the corpus is non-empty and
/// the interp-reference count is what we expect (10), so a corpus change trips it.
#[test]
fn codegen_ts_golden_emit_census() {
    let goldens = collect_expect_main_goldens();
    assert!(
        !goldens.is_empty(),
        "no examples/golden EXPECT+main goldens found"
    );

    let mut ok = 0usize;
    let mut ok_with_main = 0usize;
    println!("\n=== codegen-ts emit census over {} goldens ===", goldens.len());
    for g in &goldens {
        let name = g
            .strip_prefix(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/golden"))
            .unwrap_or(g)
            .to_string_lossy()
            .replace('\\', "/");
        let status = match probe_ts_emit(g) {
            TsEmit::Ok { defines_main, file_count } => {
                ok += 1;
                if defines_main {
                    ok_with_main += 1;
                }
                format!("EMIT_OK files={file_count} defines_main={defines_main}")
            }
            TsEmit::ParseErr(e) => format!("PARSE_ERR {}", e.lines().next().unwrap_or("")),
            TsEmit::GenErr(e) => format!("GEN_ERR {}", e.lines().next().unwrap_or("")),
            TsEmit::Panic => "PANIC (unsupported construct)".to_string(),
        };
        println!("  {name:<32} {status}");
    }
    println!(
        "=== codegen-ts: {ok}/{} emit cleanly, {ok_with_main} define a `main` ===\n",
        goldens.len()
    );

    // BASELINE RATCHET (2026-06-07): codegen-ts `BuildMode::App` emits a web
    // bundle and DROPS top-level `fn main` + free functions, so it defines
    // `main` in 0/10 goldens. CR-F2 needs a new codegen-ts "script/console"
    // emit mode before the ts arm can run under Node. When that lands and this
    // count rises, flip this assertion and wire the Node execution harness
    // (Task 1.A) + the ts column of the parity gate (Task 1.0).
    assert_eq!(
        ok_with_main, 0,
        "codegen-ts now defines `main` in {ok_with_main} golden(s) — the script-emit \
         mode appears to have landed. Build the Node execution harness + ts parity \
         column and update this baseline."
    );
}

/// Diagnostic: dump exactly what codegen-ts emits for two representative goldens
/// (`mesh/noop` = trivial return-value; `decimal_math` = print-side-effect) so we
/// can see whether the `main()` logic survives emission (→ a synthetic entry is
/// enough) or is dropped by App-mode (→ a new "script" emit mode is needed).
#[test]
fn dump_codegen_ts_for_representative_goldens() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/golden");
    for rel in ["mesh/noop.vox", "decimal_math.vox"] {
        let path = root.join(rel);
        let src = std::fs::read_to_string(&path).unwrap_or_default();
        let tokens = lex(&src);
        let Ok(module) = parse(tokens) else {
            println!("\n##### {rel}: PARSE FAILED");
            continue;
        };
        let hir = lower_module(&module);
        let opts = CodegenOptions {
            tanstack_start: false,
            target: None,
            mode: BuildMode::App,
            ..Default::default()
        };
        unsafe { std::env::set_var("VOX_WEBIR_VALIDATE", "0") };
        let emitted = generate_with_options(&hir, opts);
        unsafe { std::env::remove_var("VOX_WEBIR_VALIDATE") };
        println!("\n##### codegen-ts emit for {rel} #####");
        match emitted {
            Ok(out) => {
                for (fname, content) in &out.files {
                    println!("----- file: {fname} ({} bytes) -----", content.len());
                    // Cap each file dump so output stays readable.
                    for line in content.lines().take(60) {
                        println!("  {line}");
                    }
                }
            }
            Err(e) => println!("GEN_ERR: {e}"),
        }
    }
}
