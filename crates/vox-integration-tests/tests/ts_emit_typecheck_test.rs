#![allow(clippy::field_reassign_with_default)]
//! CI gate: compile Vox golden fixtures → TypeScript, then run `tsc --noEmit` to verify
//! that the emitted TS is type-correct.
//!
//! Marked `#[ignore]` by default — only runs in environments that have `node` / `npx` in PATH
//! (CI installs Node; local developers opt-in with `cargo test -- --ignored`).
//!
//! Run explicitly:
//!   cargo test -p vox-integration-tests --test ts_emit_typecheck_test -- --ignored --nocapture
#![allow(missing_docs)]

use std::path::PathBuf;

use rayon::prelude::*;
use vox_codegen::codegen_ts::emitter::BuildMode;
use vox_codegen::codegen_ts::{CodegenOptions, generate_with_options};
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;
use vox_integration_tests::{
    EnvVarGuard, collect_vox_files, run_tsc_noemit, strip_unc_prefix, ts_scratch_dir,
};

/// Absolute path to the `examples/golden-ts/` directory of Vox fixtures.
fn golden_ts_dir() -> PathBuf {
    strip_unc_prefix(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/golden-ts")
            .canonicalize()
            .expect("examples/golden-ts directory must exist"),
    )
}

/// Compile one `.vox` source string to TypeScript files using the codegen pipeline.
/// Returns `Vec<(filename, content)>` of emitted `.ts` / `.tsx` / `.json` files.
///
/// Caller is responsible for `VOX_WEBIR_VALIDATE` — it's process-global state, so it
/// must be set once outside any parallel loop over fixtures, not toggled per-call
/// (toggling per-call would race when called from multiple threads).
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
    let output = generate_with_options(&hir, opts)
        .unwrap_or_else(|e| panic!("Codegen failed for {label}: {e}"));
    output.files
}

/// The main test: for every `.vox` file in `examples/golden-ts/`, emit TS and verify
/// that `tsc --noEmit` succeeds.
#[test]
#[ignore = "requires node/npx in PATH; run explicitly with: cargo test -p vox-integration-tests --test ts_emit_typecheck_test -- --ignored --nocapture — owner: integration-tests sunset: 2026-12-31"]
fn all_golden_fixtures_emit_valid_typescript() {
    let scratch = ts_scratch_dir();
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

    // Set once for the whole batch, not per-fixture: VOX_WEBIR_VALIDATE is process-global
    // state, so toggling it inside the parallel loop below would race across threads.
    // Disables the WebIR validate gate for test isolation (same pattern as pipeline_test.rs)
    // — we care about whether the emitted TS type-checks, not the structural IR gate.
    // EnvVarGuard also serializes against admin_output_typechecks_when_gated (below), which
    // mutates the same var, and restores it even if a fixture panics inside the batch.
    let emitted: Vec<(String, Vec<(String, String)>)> = {
        let _guard = EnvVarGuard::set(&[("VOX_WEBIR_VALIDATE", "0")]);

        // Compile every fixture in parallel — each is an independent lex/parse/lower/codegen
        // pass with no shared mutable state (the env var is set once, read-only from here).
        vox_files
            .par_iter()
            .map(|vox_path| {
                let label = vox_path.file_stem().unwrap().to_string_lossy().to_string();
                let src = std::fs::read_to_string(vox_path)
                    .unwrap_or_else(|e| panic!("Could not read {}: {e}", vox_path.display()));
                let ts_files = compile_to_ts(&src, &label);
                (label, ts_files)
            })
            .collect()
    };

    // Write all emitted files, prefixed by fixture name to avoid collisions.
    for (label, ts_files) in &emitted {
        // Only write TypeScript/TSX files — skip JSON, Dockerfile, etc. which tsc won't type-check.
        for (name, content) in ts_files {
            if name.ends_with(".ts") || name.ends_with(".tsx") {
                // Namespace by fixture to prevent inter-fixture name collisions.
                let dest_dir = emit_dir.join(label);
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
    let tsconfig_path = emit_dir.join("tsconfig.json");
    std::fs::write(
        &tsconfig_path,
        serde_json::to_string_pretty(&vox_integration_tests::strict_tsconfig_json()).unwrap(),
    )
    .expect("Failed to write tsconfig.json");

    // Emitted mobile code does `import { voxRuntime } from "@vox/runtime"`. The
    // real `@vox/runtime` is published to npm (installed by real apps); here we
    // provide a minimal ambient declaration so `tsc` resolves the import without
    // pulling in the runtime package's own source + transitive deps.
    std::fs::write(
        emit_dir.join("vox-runtime-shim.d.ts"),
        "// Test shim for the npm-published `@vox/runtime` adapter.\n\
         declare module \"@vox/runtime\" {\n\
         \x20 export const voxRuntime: any;\n\
         }\n",
    )
    .expect("Failed to write vox-runtime-shim.d.ts");

    let output = run_tsc_noemit(&scratch, &tsconfig_path);

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

/// AGH-0005 regression gate: the opt-in admin UI (`VOX_EMIT_ADMIN=1`) emits
/// TypeScript that actually type-checks. The original Track A code emitted the
/// Convex idiom (`useQuery(api.<t>.list)`, `row._id`, `api.<t>.upsert`) with no
/// imports — non-compiling — and because the feature is gated off by default,
/// the golden-fixture gate above never exercised it. This test SETS the gate and
/// a temp registry that allows the table, then runs `tsc --noEmit` over the
/// emitted admin output. See docs/superpowers/antigravity-handoff-ledger.md §C
/// AGH-0005.
#[test]
#[ignore = "requires node/npx in PATH; run explicitly with: cargo test -p vox-integration-tests --test ts_emit_typecheck_test -- --ignored --nocapture — owner: integration-tests sunset: 2026-12-31"]
fn admin_output_typechecks_when_gated() {
    use vox_compiler::hir::nodes::DefId;
    use vox_compiler::hir::{HirModule, HirTable, HirTableField, HirType};

    let scratch = ts_scratch_dir();
    assert!(
        scratch.join("node_modules").exists(),
        "node_modules missing in ts-noemit-scratch/. Run: pnpm install --frozen-lockfile (from that directory)"
    );

    // Build HIR directly: `table User { name: string, email: email }`.
    let span = vox_compiler::ast::span::Span::new(0, 0);
    let table = HirTable {
        id: DefId(0),
        name: "User".into(),
        fields: vec![
            HirTableField {
                name: "name".into(),
                type_ann: HirType::Named("string".into()),
                span,
            },
            HirTableField {
                name: "email".into(),
                type_ann: HirType::Named("string".into()),
                span,
            },
        ],
        primary_key: None,
        is_extern: false,
        source: None,
        is_pub: true,
        is_deprecated: false,
        span,
    };
    let hir = HirModule {
        tables: vec![table],
        ..Default::default()
    };

    // Isolated emit dir + a temp registry that opts the table in.
    let emit_dir = scratch.join("__admin_emit_test__");
    if emit_dir.exists() {
        std::fs::remove_dir_all(&emit_dir).expect("clean __admin_emit_test__");
    }
    std::fs::create_dir_all(&emit_dir).expect("create __admin_emit_test__");
    let registry_path = emit_dir.join("admin-registry.yaml");
    std::fs::write(&registry_path, "admin_tables:\n  - User\n").expect("write registry");

    // Enable the gate + point at our registry; disable the WebIR validate gate for
    // isolation (same pattern as compile_to_ts). EnvVarGuard serializes against
    // all_golden_fixtures_emit_valid_typescript (above), which also mutates
    // VOX_WEBIR_VALIDATE, and restores every var even if codegen panics.
    let registry_path_str = registry_path.to_str().expect("registry path must be UTF-8");
    let output = {
        let _guard = EnvVarGuard::set(&[
            ("VOX_EMIT_ADMIN", "1"),
            ("VOX_ADMIN_REGISTRY", registry_path_str),
            ("VOX_WEBIR_VALIDATE", "0"),
        ]);
        let opts = CodegenOptions {
            tanstack_start: false,
            target: None,
            mode: BuildMode::App,
            ..Default::default()
        };
        generate_with_options(&hir, opts).expect("admin codegen")
    };

    // Sanity: the admin component is present and NOT the regressed Convex idiom.
    let forms = output
        .files
        .iter()
        .find(|(n, _)| n == "forms.tsx")
        .map(|(_, c)| c.clone())
        .unwrap_or_default();
    assert!(
        forms.contains("export function UserList()"),
        "admin output not emitted under VOX_EMIT_ADMIN=1:\n{forms}"
    );
    assert!(
        !forms.contains("useQuery(api"),
        "regressed to the Convex idiom:\n{forms}"
    );

    // Write all .ts/.tsx + a strict tsconfig, then type-check.
    for (name, content) in &output.files {
        if name.ends_with(".ts") || name.ends_with(".tsx") {
            std::fs::write(emit_dir.join(name), content)
                .unwrap_or_else(|e| panic!("write {name}: {e}"));
        }
    }
    let tsconfig_path = emit_dir.join("tsconfig.json");
    std::fs::write(
        &tsconfig_path,
        serde_json::to_string_pretty(&vox_integration_tests::strict_tsconfig_json()).unwrap(),
    )
    .expect("write tsconfig.json");

    let tsc = run_tsc_noemit(&scratch, &tsconfig_path);
    let stdout = String::from_utf8_lossy(&tsc.stdout);
    let stderr = String::from_utf8_lossy(&tsc.stderr);
    assert!(
        tsc.status.success(),
        "tsc --noEmit failed over gated admin output.\nExit: {:?}\nstdout:\n{stdout}\nstderr:\n{stderr}\nEmitted in: {}",
        tsc.status.code(),
        emit_dir.display()
    );

    let _ = std::fs::remove_dir_all(&emit_dir);
    println!("tsc --noEmit passed for gated admin output.");
}
