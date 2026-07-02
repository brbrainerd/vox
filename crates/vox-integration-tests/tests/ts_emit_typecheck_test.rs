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
        // vox-arch-check: allow shell-spawn
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

/// Resolve the local `tsc` binary (preferring `node_modules/.bin`) and run
/// `tsc --noEmit --project <tsconfig>` from `scratch`. Shared by the gated-admin
/// test below; mirrors the inline invocation in the golden-fixture test.
fn run_tsc_noemit(scratch: &Path, tsconfig_path: &Path) -> std::process::Output {
    let tsc_bin = {
        let local_tsc_cmd = scratch.join("node_modules").join(".bin").join("tsc.cmd");
        let local_tsc = scratch.join("node_modules").join(".bin").join("tsc");
        if cfg!(target_os = "windows") && local_tsc_cmd.exists() {
            local_tsc_cmd
        } else if local_tsc.exists() {
            local_tsc
        } else {
            PathBuf::from("npx")
        }
    };
    if cfg!(target_os = "windows") && tsc_bin.extension().is_some_and(|e| e == "cmd") {
        // vox-arch-check: allow shell-spawn
        Command::new("cmd")
            .arg("/C")
            .arg(&tsc_bin)
            .arg("--noEmit")
            .arg("--project")
            .arg(tsconfig_path)
            .current_dir(scratch)
            .output()
            .expect("Failed to spawn tsc.cmd — is node/pnpm installed in ts-noemit-scratch/?")
    } else {
        Command::new(&tsc_bin)
            .arg("--noEmit")
            .arg("--project")
            .arg(tsconfig_path)
            .current_dir(scratch)
            .output()
            .expect("Failed to spawn tsc — is node/pnpm installed in ts-noemit-scratch/?")
    }
}

/// Strict tsconfig (matches the golden-fixture test) for an isolated emit dir.
fn emit_tsconfig_json() -> serde_json::Value {
    serde_json::json!({
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
    })
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

    let scratch = scratch_dir();
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
    let mut hir = HirModule::default();
    hir.tables = vec![table];

    // Isolated emit dir + a temp registry that opts the table in.
    let emit_dir = scratch.join("__admin_emit_test__");
    if emit_dir.exists() {
        std::fs::remove_dir_all(&emit_dir).expect("clean __admin_emit_test__");
    }
    std::fs::create_dir_all(&emit_dir).expect("create __admin_emit_test__");
    let registry_path = emit_dir.join("admin-registry.yaml");
    std::fs::write(&registry_path, "admin_tables:\n  - User\n").expect("write registry");

    // Enable the gate + point at our registry; disable the WebIR validate gate
    // for isolation (same pattern as compile_to_ts). nextest runs each test in
    // its own process, so these env writes don't leak across tests.
    unsafe {
        std::env::set_var("VOX_EMIT_ADMIN", "1");
        std::env::set_var("VOX_ADMIN_REGISTRY", &registry_path);
        std::env::set_var("VOX_WEBIR_VALIDATE", "0");
    }
    let opts = CodegenOptions {
        tanstack_start: false,
        target: None,
        mode: BuildMode::App,
        ..Default::default()
    };
    let result = generate_with_options(&hir, opts);
    unsafe {
        std::env::remove_var("VOX_EMIT_ADMIN");
        std::env::remove_var("VOX_ADMIN_REGISTRY");
        std::env::remove_var("VOX_WEBIR_VALIDATE");
    }
    let output = result.expect("admin codegen");

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
        serde_json::to_string_pretty(&emit_tsconfig_json()).unwrap(),
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
