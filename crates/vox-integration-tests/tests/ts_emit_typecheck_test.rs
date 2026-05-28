//! CI gate: compile Vox golden fixtures → TypeScript, then run `tsc --noEmit` to verify
//! that the emitted TS is type-correct.
//!
//! Marked `#[ignore]` by default — only runs in environments that have `node` / `npx` in PATH
//! (CI installs Node; local developers opt-in with `cargo test -- --ignored`).
//!
//! Run explicitly:
//!   cargo test -p vox-integration-tests --test ts_emit_typecheck_test -- --ignored --nocapture
#![allow(missing_docs)]
#![allow(unsafe_code)] // set_var/remove_var used to isolate VOX_WEBIR_VALIDATE for this test

use std::path::{Path, PathBuf};
use std::process::Command;

use vox_codegen::codegen_ts::emitter::BuildMode;
use vox_codegen::codegen_ts::{CodegenOptions, generate_with_options};
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

/// Strip the Windows `\\?\` UNC prefix that `canonicalize()` adds on Windows.
/// `cmd.exe` and many CLI tools cannot handle the extended-length path prefix.
fn strip_unc_prefix(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        p
    }
}

/// Absolute path to the scratch dir that contains `node_modules` and the base `tsconfig.json`.
fn scratch_dir() -> PathBuf {
    strip_unc_prefix(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("ts-noemit-scratch")
            .canonicalize()
            .expect("ts-noemit-scratch directory must exist"),
    )
}

/// Absolute path to the `examples/golden-ts/` directory of Vox fixtures.
fn golden_ts_dir() -> PathBuf {
    strip_unc_prefix(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/golden-ts")
            .canonicalize()
            .expect("examples/golden-ts directory must exist"),
    )
}

/// Collect all `.vox` files from `dir`.
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

/// Compile one `.vox` source string to TypeScript files using the codegen pipeline.
/// Returns `Vec<(filename, content)>` of emitted `.ts` / `.tsx` / `.json` files.
fn compile_to_ts(src: &str, label: &str) -> Vec<(String, String)> {
    let tokens = lex(src);
    let module = parse(tokens).unwrap_or_else(|e| {
        panic!("Parse failed for {label}: {e:?}");
    });
    let hir = lower_module(&module);
    let opts = CodegenOptions {
        tanstack_start: false,
        target: None,
        mode: BuildMode::App,
        ..Default::default()
    };
    // Disable WebIR validate gate for test isolation (same pattern as pipeline_test.rs).
    // We care about whether the emitted TS type-checks, not the structural IR gate.
    unsafe { std::env::set_var("VOX_WEBIR_VALIDATE", "0") };
    let output = generate_with_options(&hir, opts)
        .unwrap_or_else(|e| panic!("Codegen failed for {label}: {e}"));
    unsafe { std::env::remove_var("VOX_WEBIR_VALIDATE") };
    output.files
}

/// The main test: for every `.vox` file in `examples/golden-ts/`, emit TS and verify
/// that `tsc --noEmit` succeeds.
#[test]
#[ignore = "requires node/npx in PATH; run explicitly with: cargo test -p vox-integration-tests --test ts_emit_typecheck_test -- --ignored --nocapture — owner: integration-tests sunset: 2026-12-31"]
fn all_golden_fixtures_emit_valid_typescript() {
    let scratch = scratch_dir();
    let golden_dir = golden_ts_dir();

    // Verify node_modules exist (pnpm install must have run).
    let node_modules = scratch.join("node_modules");
    assert!(
        node_modules.exists(),
        "node_modules missing in ts-noemit-scratch/. Run: pnpm install --frozen-lockfile (from that directory)"
    );

    let vox_files = collect_vox_files(&golden_dir);
    assert!(
        !vox_files.is_empty(),
        "No .vox files found in examples/golden-ts/"
    );

    // Write emit output into ts-noemit-scratch/__emit_test__/
    let emit_dir = scratch.join("__emit_test__");
    if emit_dir.exists() {
        std::fs::remove_dir_all(&emit_dir).expect("Failed to clean __emit_test__");
    }
    std::fs::create_dir_all(&emit_dir).expect("Failed to create __emit_test__");

    // Emit all fixtures into the test dir, prefixed by fixture name to avoid collisions.
    for vox_path in &vox_files {
        let label = vox_path.file_stem().unwrap().to_string_lossy();
        let src = std::fs::read_to_string(vox_path)
            .unwrap_or_else(|e| panic!("Could not read {}: {e}", vox_path.display()));

        let ts_files = compile_to_ts(&src, &label);

        // Only write TypeScript/TSX files — skip JSON, Dockerfile, etc. which tsc won't type-check.
        for (name, content) in &ts_files {
            if name.ends_with(".ts") || name.ends_with(".tsx") {
                // Namespace by fixture to prevent inter-fixture name collisions.
                let dest_dir = emit_dir.join(label.as_ref());
                std::fs::create_dir_all(&dest_dir)
                    .unwrap_or_else(|e| panic!("mkdir {}: {e}", dest_dir.display()));
                let dest = dest_dir.join(name);
                std::fs::write(&dest, content)
                    .unwrap_or_else(|e| panic!("write {}: {e}", dest.display()));
            }
        }
    }

    // Write a per-run tsconfig into __emit_test__/ that includes all emitted files.
    // Uses compilerOptions inline (cannot use `extends` with a path that node_modules
    // resolution may not find on Windows without a junction).
    let tsconfig_content = serde_json::json!({
        "compilerOptions": {
            "target": "ES2022",
            "module": "ESNext",
            "moduleResolution": "bundler",
            "strict": true,
            "noEmit": true,
            "jsx": "react-jsx",
            "skipLibCheck": true,
            "esModuleInterop": true,
            "isolatedModules": true,
            "lib": ["ES2022", "DOM", "DOM.Iterable"]
        },
        "include": ["./**/*.ts", "./**/*.tsx"]
    });
    let tsconfig_path = emit_dir.join("tsconfig.json");
    std::fs::write(
        &tsconfig_path,
        serde_json::to_string_pretty(&tsconfig_content).unwrap(),
    )
    .expect("Failed to write tsconfig.json");

    // Resolve tsc: prefer the local node_modules/.bin/tsc (avoids PATH resolution issues
    // on Windows), falling back to npx tsc if the local binary isn't present.
    let tsc_bin = {
        let local_tsc_cmd = scratch.join("node_modules").join(".bin").join("tsc.cmd");
        let local_tsc = scratch.join("node_modules").join(".bin").join("tsc");
        if cfg!(target_os = "windows") && local_tsc_cmd.exists() {
            local_tsc_cmd
        } else if local_tsc.exists() {
            local_tsc
        } else {
            // fallback: hope tsc is in PATH
            PathBuf::from("npx")
        }
    };

    // For Windows .cmd files we must invoke via cmd.exe.
    let output = if cfg!(target_os = "windows") && tsc_bin.extension().is_some_and(|e| e == "cmd") {
        Command::new("cmd")
            .arg("/C")
            .arg(&tsc_bin)
            .arg("--noEmit")
            .arg("--project")
            .arg(&tsconfig_path)
            .current_dir(&scratch)
            .output()
            .expect("Failed to spawn tsc.cmd — is node/pnpm installed in ts-noemit-scratch/?")
    } else {
        Command::new(&tsc_bin)
            .arg("--noEmit")
            .arg("--project")
            .arg(&tsconfig_path)
            .current_dir(&scratch)
            .output()
            .expect("Failed to spawn tsc — is node/pnpm installed in ts-noemit-scratch/?")
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        panic!(
            "tsc --noEmit failed over golden-ts fixtures.\n\
             Exit code: {:?}\n\
             stdout:\n{stdout}\n\
             stderr:\n{stderr}\n\
             Emitted files are in: {emit_dir}",
            output.status.code(),
            emit_dir = emit_dir.display()
        );
    }

    // Clean up on success.
    let _ = std::fs::remove_dir_all(&emit_dir);

    println!(
        "tsc --noEmit passed for {} golden-ts fixtures.",
        vox_files.len()
    );
}
