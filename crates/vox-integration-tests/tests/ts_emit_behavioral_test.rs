//! CR-F2b: web BEHAVIORAL gate. Compile a Vox `golden-ts` fixture → TypeScript,
//! then actually RENDER the emitted React component under a DOM-free server
//! renderer (`react-dom/server`'s `renderToStaticMarkup`) and assert it mounts
//! without error and produces the expected markup.
//!
//! Pipeline per fixture: emit `.tsx` → transpile to ESM `.js` with the scratch
//! project's `tsc` (Node v24 cannot strip JSX from `.tsx` directly) → import the
//! emitted `.js` in an ESM runner and `renderToStaticMarkup` it.
//!
//! This is the behavioral complement to `ts_emit_typecheck_test.rs`: typecheck
//! proves the emitted TS is type-correct; this proves it actually executes and
//! renders. It catches runtime bugs (bad hook usage, undefined refs, broken JSX
//! children) that `tsc --noEmit` cannot see.
//!
//! SEED scope (CR-F2b first slice): one simple component fixture
//! (`component_state.vox` → `Counter`). RATCHET: extend `BEHAVIORAL_FIXTURES`
//! to cover all renderable `examples/golden-ts/` component fixtures as CR-F2b
//! matures.
//!
//! Marked `#[ignore]` by default — only runs where `node` is in PATH and the
//! scratch project's `node_modules` are installed (CI installs Node + runs
//! `pnpm install`; local developers opt in with `--ignored`).
//!
//! Run explicitly:
//!   cargo test -p vox-integration-tests --test ts_emit_behavioral_test -- --ignored --nocapture
#![allow(missing_docs)]

use std::path::PathBuf;
use std::process::Command;

use vox_codegen::codegen_ts::emitter::BuildMode;
use vox_codegen::codegen_ts::{CodegenOptions, generate_with_options};
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

/// One renderable fixture: the golden `.vox` file stem, the exported component
/// to render, and a snippet of markup the rendered output must contain.
struct BehavioralFixture {
    /// File stem under `examples/golden-ts/` (without `.vox`).
    fixture: &'static str,
    /// Name of the exported React component (emitted to `<component>.tsx`).
    component: &'static str,
    /// A substring the static-rendered HTML must contain (proves the view tree
    /// actually rendered, not just that the module loaded).
    expect_contains: &'static str,
    /// JSON props object passed to the component (for components with params).
    /// `"{}"` for prop-less components.
    props_json: &'static str,
}

/// RATCHET: add a row here for each renderable golden-ts component fixture as
/// CR-F2b matures. Keep these to fixtures whose components render without
/// runtime-only props/loaders.
const BEHAVIORAL_FIXTURES: &[BehavioralFixture] = &[
    BehavioralFixture {
        fixture: "component_state",
        component: "Counter",
        expect_contains: "Increment",
        props_json: "{}",
    },
    BehavioralFixture {
        // Stateful component with `state` + `derived` + `effect`, taking a prop.
        // Renders the derived label "Count: 7" from the `initial=7` prop.
        fixture: "component_effect_with_deps",
        component: "Timer",
        expect_contains: "Count: 7",
        props_json: r#"{ "initial": 7 }"#,
    },
];

/// Strip the Windows `\\?\` UNC prefix that `canonicalize()` adds on Windows.
fn strip_unc_prefix(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        p
    }
}

/// Absolute path to the scratch dir that contains `node_modules`.
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

/// Compile one `.vox` source string to TypeScript files using the codegen
/// pipeline. Returns `Vec<(filename, content)>` of emitted files.
/// (Same emit flow as `ts_emit_typecheck_test.rs`.)
// SAFETY: set_var/remove_var used to isolate VOX_WEBIR_VALIDATE; called from
// #[ignore] tests that are single-threaded with respect to this env var.
#[allow(unsafe_code)]
fn compile_to_ts(src: &str, label: &str) -> Vec<(String, String)> {
    let tokens = lex(src);
    let module = parse(tokens).unwrap_or_else(|e| panic!("Parse failed for {label}: {e:?}"));
    let hir = lower_module(&module);
    let opts = CodegenOptions {
        tanstack_start: false,
        target: None,
        mode: BuildMode::App,
        ..Default::default()
    };
    unsafe { std::env::set_var("VOX_WEBIR_VALIDATE", "0") };
    let output = generate_with_options(&hir, opts)
        .unwrap_or_else(|e| panic!("Codegen failed for {label}: {e}"));
    unsafe { std::env::remove_var("VOX_WEBIR_VALIDATE") };
    output.files
}

/// Resolve the scratch project's local `tsc` binary (preferred over PATH `npx`).
fn tsc_command(scratch: &PathBuf) -> Command {
    let local_tsc_cmd = scratch.join("node_modules").join(".bin").join("tsc.cmd");
    let local_tsc = scratch.join("node_modules").join(".bin").join("tsc");
    if cfg!(target_os = "windows") && local_tsc_cmd.exists() {
        // vox-arch-check: allow shell-spawn
        let mut c = Command::new("cmd");
        c.arg("/C").arg(local_tsc_cmd);
        c
    } else if local_tsc.exists() {
        Command::new(local_tsc)
    } else {
        let mut c = Command::new("npx");
        c.arg("tsc");
        c
    }
}

/// Resolve the `node` binary, preferring PATH; spawns via cmd.exe on Windows so
/// a `node.exe`/`node.cmd` shim resolves the same way the typecheck test
/// resolves `tsc`.
fn run_node_esm(script_path: &PathBuf, cwd: &PathBuf) -> std::process::Output {
    if cfg!(target_os = "windows") {
        // vox-arch-check: allow shell-spawn
        Command::new("cmd")
            .arg("/C")
            .arg("node")
            .arg(script_path)
            .current_dir(cwd)
            .output()
            .expect("Failed to spawn node — is Node installed and on PATH?")
    } else {
        Command::new("node")
            .arg(script_path)
            .current_dir(cwd)
            .output()
            .expect("Failed to spawn node — is Node installed and on PATH?")
    }
}

/// CR-F2b SEED: emit each behavioral fixture, render its component under
/// `react-dom/server`, and assert the rendered markup is non-empty and contains
/// the expected element/text.
#[test]
#[ignore = "requires node in PATH + ts-noemit-scratch/node_modules; run explicitly with: cargo test -p vox-integration-tests --test ts_emit_behavioral_test -- --ignored --nocapture — owner: integration-tests sunset: 2026-12-31"]
fn golden_components_render_to_expected_markup() {
    let scratch = scratch_dir();
    let golden_dir = golden_ts_dir();

    // node_modules must exist (pnpm install must have run).
    let node_modules = scratch.join("node_modules");
    assert!(
        node_modules.exists(),
        "node_modules missing in ts-noemit-scratch/. Run: pnpm install --frozen-lockfile (from that directory)"
    );

    // Render output into ts-noemit-scratch/__behavioral_test__/ (must live
    // inside the scratch project so `react`/`react-dom` resolve from its
    // node_modules).
    let work_dir = scratch.join("__behavioral_test__");
    if work_dir.exists() {
        std::fs::remove_dir_all(&work_dir).expect("Failed to clean __behavioral_test__");
    }
    std::fs::create_dir_all(&work_dir).expect("Failed to create __behavioral_test__");

    // A package.json with `"type": "module"` so node runs the `.mjs`-style
    // runner as ESM (matching the emitted ESM imports).
    std::fs::write(
        work_dir.join("package.json"),
        "{\n  \"type\": \"module\"\n}\n",
    )
    .expect("Failed to write __behavioral_test__/package.json");

    for f in BEHAVIORAL_FIXTURES {
        let vox_path = golden_dir.join(format!("{}.vox", f.fixture));
        let src = std::fs::read_to_string(&vox_path)
            .unwrap_or_else(|e| panic!("Could not read {}: {e}", vox_path.display()));

        let ts_files = compile_to_ts(&src, f.fixture);

        // Per-fixture subdir to avoid cross-fixture filename collisions.
        // `src/` holds the emitted `.tsx`; `js/` holds the tsc-transpiled ESM.
        let fixture_dir = work_dir.join(f.fixture);
        let src_dir = fixture_dir.join("src");
        let js_dir = fixture_dir.join("js");
        std::fs::create_dir_all(&src_dir)
            .unwrap_or_else(|e| panic!("mkdir {}: {e}", src_dir.display()));

        let mut emitted_component_tsx = false;
        let mut written_sources: Vec<PathBuf> = Vec::new();
        for (name, content) in &ts_files {
            if name.ends_with(".ts") || name.ends_with(".tsx") {
                let dest = src_dir.join(name);
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent).ok();
                }
                std::fs::write(&dest, content)
                    .unwrap_or_else(|e| panic!("write {}: {e}", dest.display()));
                written_sources.push(dest);
                if name == &format!("{}.tsx", f.component) {
                    emitted_component_tsx = true;
                }
            }
        }

        assert!(
            emitted_component_tsx,
            "Fixture `{}` did not emit `{}.tsx`. Emitted: {:?}",
            f.fixture,
            f.component,
            ts_files.iter().map(|(n, _)| n).collect::<Vec<_>>()
        );

        // Transpile the emitted TSX → ESM JS. Node v24 cannot strip JSX from a
        // `.tsx`, so we use the scratch project's `tsc`. `--isolatedModules` +
        // per-file emit (no `noEmit`) yields runnable `.js` next to the source.
        let mut tsc_cmd = tsc_command(&scratch);
        for src in &written_sources {
            tsc_cmd.arg(src);
        }
        let tsc_out = tsc_cmd
            .arg("--jsx")
            .arg("react-jsx")
            .arg("--module")
            .arg("esnext")
            .arg("--target")
            .arg("es2022")
            .arg("--moduleResolution")
            .arg("bundler")
            .arg("--skipLibCheck")
            .arg("--outDir")
            .arg(&js_dir)
            .current_dir(&scratch)
            .output()
            .expect("Failed to spawn tsc — is node/pnpm installed in ts-noemit-scratch/?");
        assert!(
            tsc_out.status.success(),
            "tsc transpile FAILED for fixture `{}`.\nstdout:\n{}\nstderr:\n{}",
            f.fixture,
            String::from_utf8_lossy(&tsc_out.stdout),
            String::from_utf8_lossy(&tsc_out.stderr),
        );

        // Generate the ESM runner: import the transpiled component `.js` and
        // render it to static markup.
        let runner_path = js_dir.join("__render__.mjs");
        let runner = format!(
            r#"import {{ createElement }} from "react";
import {{ renderToStaticMarkup }} from "react-dom/server";
import {{ {component} }} from "./{component}.js";

const props = {props_json};

let html;
try {{
  html = renderToStaticMarkup(createElement({component}, props));
}} catch (e) {{
  console.error("RENDER_ERROR:", e && e.stack ? e.stack : String(e));
  process.exit(2);
}}

if (typeof html !== "string" || html.length === 0) {{
  console.error("EMPTY_MARKUP: component `{component}` rendered to empty/non-string output");
  process.exit(3);
}}

console.log("RENDERED_HTML:" + html);
"#,
            component = f.component,
            props_json = f.props_json,
        );
        std::fs::write(&runner_path, runner)
            .unwrap_or_else(|e| panic!("write runner {}: {e}", runner_path.display()));

        let output = run_node_esm(&runner_path, &scratch);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(
            output.status.success(),
            "Behavioral render FAILED for fixture `{}` (component `{}`).\n\
             Exit code: {:?}\n\
             stdout:\n{stdout}\n\
             stderr:\n{stderr}\n\
             Emitted files are in: {}",
            f.fixture,
            f.component,
            output.status.code(),
            fixture_dir.display(),
        );

        // The runner prints `RENDERED_HTML:<markup>`; assert it contains the
        // expected element/text (proves the view tree rendered, not just that
        // the module loaded).
        let html_line = stdout
            .lines()
            .find_map(|l| l.strip_prefix("RENDERED_HTML:"))
            .unwrap_or_else(|| {
                panic!(
                    "No RENDERED_HTML line for fixture `{}`.\nstdout:\n{stdout}\nstderr:\n{stderr}",
                    f.fixture
                )
            });

        assert!(
            html_line.contains(f.expect_contains),
            "Rendered markup for fixture `{}` missing expected substring `{}`.\n\
             Rendered HTML:\n{html_line}",
            f.fixture,
            f.expect_contains,
        );

        println!(
            "CR-F2b behavioral render PASSED: `{}` → <{}/> rendered {} bytes containing `{}`.",
            f.fixture,
            f.component,
            html_line.len(),
            f.expect_contains,
        );
    }

    // Clean up on success.
    let _ = std::fs::remove_dir_all(&work_dir);

    println!(
        "CR-F2b: {} behavioral fixture(s) rendered green.",
        BEHAVIORAL_FIXTURES.len()
    );
}
