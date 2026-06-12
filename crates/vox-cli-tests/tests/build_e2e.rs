//! End-to-end `vox build` integration tests. Each test runs the CLI as a subprocess
//! against a fixture under `tests/fixtures/<name>/main.vox`, then asserts the emitted
//! output is real (exit 0, files present, tsc compiles, no panics).
//!
//! See [the harness lib](../src/lib.rs) for the assertions implemented.
//!
//! Set `VOX_CLI_TESTS_SKIP_TSC=1` to skip TypeScript compilation checks (useful when
//! Node is not available in the CI environment).
//!
//! Set `VOX_CLI_TESTS_SKIP_CARGO=1` to skip `cargo check` of generated Rust backends.

use vox_cli_tests::BuildRun;

/// No-op kept for backward compatibility with earlier harness iterations that needed
/// to capture the vox binary path from `env!("CARGO_BIN_EXE_vox")`. The current
/// harness derives the path from the workspace root deterministically.
fn init_vox_binary_once() {}

/// The original regression fixture — `examples/golden-ts/component_state.vox` was the
/// source on which `vox build` panicked at `main_boot.rs:288` until the 2026-05-28 fix.
/// This test is the regression gate that catches a recurrence of that class of bug.
#[test]
fn build_component_state_succeeds_end_to_end() {
    init_vox_binary_once();
    let run = BuildRun::run("component_state");
    run.assert_success();
    run.assert_no_panic();
    run.assert_expected_files();
}

/// Same fixture, with the heavier downstream gates (`tsc --noEmit`,
/// `cargo check` of the generated backend) enabled. Separated so a quick
/// run without Node still validates the core regression gate.
#[test]
fn build_component_state_emits_valid_typescript_and_rust() {
    init_vox_binary_once();
    let run = BuildRun::run("component_state");
    run.assert_all();
}

/// `@form` declarations must emit `forms.tsx` + the `vox-client.ts` typed SDK.
#[test]
fn build_form_basic_succeeds_end_to_end() {
    init_vox_binary_once();
    let run = BuildRun::run("form_basic");
    run.assert_success();
    run.assert_no_panic();
    run.assert_expected_files();
}

/// `routes { ... }` declarations must emit a TanStack Router manifest plus
/// per-component .tsx files for each route entry.
#[test]
fn build_routes_with_loader_succeeds_end_to_end() {
    init_vox_binary_once();
    let run = BuildRun::run("routes_with_loader");
    run.assert_success();
    run.assert_no_panic();
    run.assert_expected_files();
}

/// `@back_button` must emit a `mobile.ts` adapter calling into the runtime layer.
#[test]
fn build_mobile_back_button_succeeds_end_to_end() {
    init_vox_binary_once();
    let run = BuildRun::run("mobile_back_button");
    run.assert_success();
    run.assert_no_panic();
    run.assert_expected_files();
}

/// `vox build --target=mobile` must produce a working React Native + Expo project:
/// per-component RN TSX, Expo scaffolding (app.json, metro.config.js, eas.json, etc.),
/// and NO Axum Rust backend (mobile apps don't ship one).
#[test]
fn build_mobile_counter_succeeds_end_to_end() {
    init_vox_binary_once();
    let run = BuildRun::run("mobile_counter");
    run.assert_success();
    run.assert_no_panic();
    run.assert_expected_files();
}

/// Heavy gate: the emitted RN TSX must type-check via `tsc --noEmit` against the
/// shared `@types/react-native` declarations. Catches regressions where the RN
/// emit produces TSX that "looks right" but won't compile in a real Expo project.
#[test]
fn build_mobile_counter_emits_valid_react_native_typescript() {
    init_vox_binary_once();
    let run = BuildRun::run("mobile_counter");
    run.assert_success();
    run.assert_no_panic();
    run.assert_expected_files();
    run.assert_tsc_compiles();
}

/// `for item, i in items key=item { panel() { text() { item } } }` must lower
/// to `{items.map((item, i) => (<View key={item}><Text>{item}</Text></View>))}`
/// — pure RN, no DOM tags. Regression gate for the original split-brain bug
/// where loop bodies fell through to React-DOM emit inside an RN component.
#[test]
fn build_mobile_list_renders_pure_rn_inside_for_loop() {
    init_vox_binary_once();
    let run = BuildRun::run("mobile_list");
    run.assert_success();
    run.assert_no_panic();
    run.assert_expected_files();
    let todo_tsx =
        std::fs::read_to_string(run.out_dir.path().join("TodoList.tsx")).expect("TodoList.tsx");
    // The loop must use `.map((item, i: number) => ...)` over the iterator.
    assert!(
        todo_tsx.contains("items.map("),
        "for-loop must lower to `.map(...)`; got:\n{todo_tsx}"
    );
    // The body must use RN tags. No React DOM leakage allowed.
    assert!(
        !todo_tsx.contains("<div"),
        "loop body must NOT contain `<div>` (split-brain regression):\n{todo_tsx}"
    );
    assert!(
        !todo_tsx.contains("<p>") && !todo_tsx.contains("<p "),
        "loop body must NOT contain `<p>` (split-brain regression):\n{todo_tsx}"
    );
    assert!(
        !todo_tsx.contains("className"),
        "loop body must NOT contain `className` (RN ignores it):\n{todo_tsx}"
    );
    // The body MUST use RN primitives.
    assert!(
        todo_tsx.contains("<View") && todo_tsx.contains("<Text"),
        "loop body must use `<View>` and `<Text>`; got:\n{todo_tsx}"
    );
    // Key injection must produce `key={item}` on the first body element.
    assert!(
        todo_tsx.contains("key={item}"),
        "loop must inject `key={{item}}` on first body element; got:\n{todo_tsx}"
    );
}

/// Heavy gate for the list fixture: tsc must accept the loop output.
#[test]
fn build_mobile_list_emits_valid_react_native_typescript() {
    init_vox_binary_once();
    let run = BuildRun::run("mobile_list");
    run.assert_success();
    run.assert_no_panic();
    run.assert_tsc_compiles();
}

/// `@form` must produce a pure RN form on `--target=mobile`:
/// View / Text / TextInput / Pressable (NOT `<form>` / `<input>` / `<button>`).
/// Same validation logic shape as the web emit — proven by asserting the same
/// pattern in both outputs in `mobile_and_web_form_share_validation_logic`.
#[test]
fn build_mobile_form_produces_pure_rn_form() {
    init_vox_binary_once();
    let run = BuildRun::run("mobile_form");
    run.assert_success();
    run.assert_no_panic();
    run.assert_expected_files();
    let forms_tsx =
        std::fs::read_to_string(run.out_dir.path().join("forms.tsx")).expect("forms.tsx");
    // Must use RN primitives.
    assert!(
        forms_tsx.contains("from \"react-native\""),
        "mobile forms.tsx must import from `react-native`; got:\n{forms_tsx}"
    );
    assert!(
        forms_tsx.contains("<TextInput") && forms_tsx.contains("<Pressable"),
        "mobile forms.tsx must use `<TextInput>` and `<Pressable>`; got:\n{forms_tsx}"
    );
    assert!(
        forms_tsx.contains("onChangeText="),
        "mobile forms.tsx must use `onChangeText`, not `onChange`; got:\n{forms_tsx}"
    );
    // Must NOT use DOM tags.
    assert!(
        !forms_tsx.contains("<form ") && !forms_tsx.contains("<form>"),
        "mobile forms.tsx must NOT use `<form>`; got:\n{forms_tsx}"
    );
    assert!(
        !forms_tsx.contains("<input "),
        "mobile forms.tsx must NOT use `<input>`; got:\n{forms_tsx}"
    );
    assert!(
        !forms_tsx.contains("className="),
        "mobile forms.tsx must NOT use `className` (RN ignores it); got:\n{forms_tsx}"
    );
    assert!(
        !forms_tsx.contains("ev.preventDefault"),
        "mobile forms.tsx must NOT call `preventDefault` (no synthetic events on RN); got:\n{forms_tsx}"
    );
    // Validation logic must use the same error-key shape as the web emit.
    assert!(
        forms_tsx.contains("e.name = \"Item name is required\""),
        "validation must produce the same error key as the web emit; got:\n{forms_tsx}"
    );
}

/// Cross-target parity: the validation function in the RN form output uses
/// the SAME error keys + message text + shape as the web form output. Drift
/// here would mean a Vox source produces subtly different validation across
/// targets — exactly the split-brain bug the single-HIR design exists to prevent.
#[test]
fn mobile_and_web_form_share_validation_logic() {
    init_vox_binary_once();
    let mobile = BuildRun::run("mobile_form");
    mobile.assert_success();
    let web = BuildRun::run("form_basic");
    web.assert_success();

    let mobile_tsx =
        std::fs::read_to_string(mobile.out_dir.path().join("forms.tsx")).expect("mobile forms.tsx");
    let web_tsx =
        std::fs::read_to_string(web.out_dir.path().join("forms.tsx")).expect("web forms.tsx");

    // The required-field validation message must be identical character-for-character.
    let validation_line = "if (name === undefined || name === null || name === \"\") e.name = \"Item name is required\"";
    assert!(
        mobile_tsx.contains(validation_line),
        "mobile validation must contain `{validation_line}`; got:\n{mobile_tsx}"
    );
    assert!(
        web_tsx.contains(validation_line),
        "web validation must contain `{validation_line}`; got:\n{web_tsx}"
    );

    // The submit call shape must be identical (single-arg object with field names).
    let submit_call = "await submit_item({ name })";
    assert!(
        mobile_tsx.contains(submit_call),
        "mobile submit must call `{submit_call}`; got:\n{mobile_tsx}"
    );
    assert!(
        web_tsx.contains(submit_call),
        "web submit must call `{submit_call}`; got:\n{web_tsx}"
    );
}

/// Heavy gate for the form fixture.
#[test]
fn build_mobile_form_emits_valid_react_native_typescript() {
    init_vox_binary_once();
    let run = BuildRun::run("mobile_form");
    run.assert_success();
    run.assert_no_panic();
    run.assert_tsc_compiles();
}

/// `routes { ... }` must lower to Expo Router file-system routes:
///   `"/"`           -> `app/index.tsx`
///   `"/about"`      -> `app/about.tsx`
///   `"/detail/:id"` -> `app/detail/[id].tsx`
/// Plus a root `app/_layout.tsx` and a package.json with `main:
/// "expo-router/entry"` and the expo-router dep chain pinned.
#[test]
fn build_mobile_routes_emits_expo_router_file_tree() {
    init_vox_binary_once();
    let run = BuildRun::run("mobile_routes");
    run.assert_success();
    run.assert_no_panic();
    run.assert_expected_files();

    let layout = std::fs::read_to_string(run.out_dir.path().join("app/_layout.tsx"))
        .expect("app/_layout.tsx");
    assert!(
        layout.contains("from \"expo-router\""),
        "_layout must import from `expo-router`; got:\n{layout}"
    );
    assert!(
        layout.contains("<Stack"),
        "_layout must render <Stack>; got:\n{layout}"
    );

    let index =
        std::fs::read_to_string(run.out_dir.path().join("app/index.tsx")).expect("app/index.tsx");
    assert!(
        index.contains("export { Home as default } from \"../Home\""),
        "app/index.tsx must re-export Home from `../Home`; got:\n{index}"
    );

    let detail = std::fs::read_to_string(run.out_dir.path().join("app/detail/[id].tsx"))
        .expect("app/detail/[id].tsx");
    assert!(
        detail.contains("export { Detail as default } from \"../../Detail\""),
        "nested detail/[id].tsx must use `../../Detail` for double-depth: got:\n{detail}"
    );

    let pkg =
        std::fs::read_to_string(run.out_dir.path().join("package.json")).expect("package.json");
    assert!(
        pkg.contains("\"main\": \"expo-router/entry\""),
        "package.json `main` must switch to `expo-router/entry`; got:\n{pkg}"
    );
    assert!(
        pkg.contains("\"expo-router\""),
        "package.json must include the expo-router dep; got:\n{pkg}"
    );

    let app_json = std::fs::read_to_string(run.out_dir.path().join("app.json")).expect("app.json");
    assert!(
        app_json.contains("\"plugins\": [\"expo-router\"]"),
        "app.json must register `expo-router` in plugins; got:\n{app_json}"
    );
}

/// Mental-tracker-shape proving ground: a single Vox source exercising the
/// full union of RN-supported VUV vocabulary. Asserts the build produces
/// every expected file and that the critical RN-specific behaviors
/// (custom-component refs, mobile-utils auto-emit + auto-import, arrow
/// handlers without IIFE) all work end-to-end.
#[test]
fn build_mobile_app_complete_exercises_full_rn_vocabulary() {
    init_vox_binary_once();
    let run = BuildRun::run("mobile_app_complete");
    run.assert_success();
    run.assert_no_panic();
    run.assert_expected_files();

    let home = std::fs::read_to_string(run.out_dir.path().join("Home.tsx")).expect("Home.tsx");
    // Custom-component invocation inside a for-loop body must render as a
    // JSX tag with the original PascalCase name and a `{...}` attr per arg,
    // NOT fall through to an empty `<View>`.
    assert!(
        home.contains("<EntryCard label={item}"),
        "for-loop body must render `<EntryCard label={{item}}/>`; got:\n{home}"
    );
    // mobile-utils import must be auto-added when the component body
    // references the `mobile` identifier.
    assert!(
        home.contains("import { mobile } from \"./mobile-utils\""),
        "component using `mobile` must auto-import from `./mobile-utils`; got:\n{home}"
    );
    // Arrow handler must NOT be triple-wrapped IIFE; the strip+wrap logic
    // produces a single clean `() => (mobile.notify(...))`.
    assert!(
        !home.contains("(() => ("),
        "arrow handler must not be IIFE-wrapped (split-brain regression):\n{home}"
    );
    assert!(
        home.contains("onPress={() => (mobile.notify("),
        "mobile.notify handler must lower to a clean arrow:\n{home}"
    );

    // mobile-utils.ts must route through voxRuntime, not Tauri directly.
    let utils = std::fs::read_to_string(run.out_dir.path().join("mobile-utils.ts"))
        .expect("mobile-utils.ts");
    assert!(
        utils.contains("from \"@vox/runtime-rn\""),
        "mobile-utils.ts must import from `@vox/runtime-rn`; got:\n{utils}"
    );
    assert!(
        !utils.contains("@tauri-apps/api"),
        "mobile-utils.ts must NOT import Tauri APIs directly:\n{utils}"
    );
    // Snake_case Vox method names must remain present (the bridge handles
    // case translation to camelCase voxRuntime methods).
    assert!(
        utils.contains("transcribe_microphone")
            && utils.contains("voxRuntime.transcribeMicrophone"),
        "mobile-utils.ts must bridge snake_case `transcribe_microphone` to camelCase `voxRuntime.transcribeMicrophone`; got:\n{utils}"
    );

    // The Entry component (uses transcribe_microphone) must also auto-import.
    let entry = std::fs::read_to_string(run.out_dir.path().join("Entry.tsx")).expect("Entry.tsx");
    assert!(
        entry.contains("import { mobile } from \"./mobile-utils\""),
        "Entry.tsx must auto-import mobile (uses transcribe_microphone):\n{entry}"
    );

    // The Detail component (no mobile use, no state) must NOT auto-import mobile.
    let detail =
        std::fs::read_to_string(run.out_dir.path().join("Detail.tsx")).expect("Detail.tsx");
    assert!(
        !detail.contains("from \"./mobile-utils\""),
        "Detail.tsx (no mobile use) must NOT import mobile-utils:\n{detail}"
    );
}

// NOTE: no `assert_tsc_compiles` for mobile_routes. The expo-router peer-dep
// chain requires a specific Expo SDK-tier version pin (RN, react-native-screens,
// expo-linking, expo-constants, @types/react all together) that conflicts
// with the SDK 52 baseline we ship in the scaffold. Real consumer projects
// pin via `npx create-expo-app --template` and don't hit the problem. The
// fast-path test above already verifies the route file structure, the
// content of each emitted file, the package.json `main` field, and the
// app.json plugins array — sufficient regression coverage for the codegen
// pipeline without forcing the harness into Expo-SDK-version maintenance.

/// Regression gate: the same Vox source produces RN-shaped output for the mobile target
/// (View / Text / Pressable / StyleSheet) and DOM-shaped output for the default web
/// target — both from one HIR. Asserts the leaf shapes differ in the right places.
#[test]
fn mobile_and_web_emit_differ_in_leaf_shape_not_in_logic() {
    init_vox_binary_once();
    let mobile = BuildRun::run("mobile_counter");
    mobile.assert_success();
    let mobile_tsx =
        std::fs::read_to_string(mobile.out_dir.path().join("Counter.tsx")).expect("Counter.tsx");
    // RN-flavored shape: React Native primitives + StyleSheet.
    assert!(
        mobile_tsx.contains("from \"react-native\""),
        "mobile Counter.tsx must import from `react-native`; got:\n{mobile_tsx}"
    );
    assert!(
        mobile_tsx.contains("<View"),
        "mobile Counter.tsx must use `<View>`; got:\n{mobile_tsx}"
    );
    assert!(
        mobile_tsx.contains("<Pressable"),
        "mobile Counter.tsx must use `<Pressable>`; got:\n{mobile_tsx}"
    );
    assert!(
        mobile_tsx.contains("StyleSheet.create"),
        "mobile Counter.tsx must call `StyleSheet.create`; got:\n{mobile_tsx}"
    );
    assert!(
        !mobile_tsx.contains("className"),
        "mobile Counter.tsx must not use `className` (RN ignores it); got:\n{mobile_tsx}"
    );
    // State mutation must be rewritten to setter calls.
    assert!(
        mobile_tsx.contains("set_n("),
        "mobile Counter.tsx must rewrite `n = expr` to `set_n(expr)`; got:\n{mobile_tsx}"
    );
    assert!(
        mobile_tsx.contains("onPress="),
        "mobile Counter.tsx must use `onPress`, not `onClick`; got:\n{mobile_tsx}"
    );

    let web = BuildRun::run("component_state");
    web.assert_success();
    let web_tsx =
        std::fs::read_to_string(web.out_dir.path().join("Counter.tsx")).expect("web Counter.tsx");
    // Web-flavored shape: React DOM + Tailwind classes.
    assert!(
        web_tsx.contains("<div"),
        "web Counter.tsx must use `<div>`; got:\n{web_tsx}"
    );
    assert!(
        web_tsx.contains("onClick="),
        "web Counter.tsx must use `onClick`; got:\n{web_tsx}"
    );
    assert!(
        web_tsx.contains("className="),
        "web Counter.tsx must use `className`; got:\n{web_tsx}"
    );
    // The state-setter rewrite is shared by both targets — split-brain gate.
    assert!(
        web_tsx.contains("set_n("),
        "web Counter.tsx must also rewrite `n = expr` to `set_n(expr)`; got:\n{web_tsx}"
    );
}

/// Regression gate: the desktop (Tauri) target must wire `@scheduled` jobs into the
/// generated `src-tauri/src/main.rs`. Before the 2026-06-03 fix, `emit_tauri_main_rs`
/// silently dropped `@scheduled` functions — they compiled into lib.rs but were never
/// registered or started, so scheduled jobs never ran on desktop (the Axum path wired
/// them, the Tauri path did not — a codegen split-brain).
///
/// This drives the same `RustAppShell::TauriApp` codegen path that
/// `vox compile --target desktop` uses (via `bundle::run` → `build::run`), but asserts
/// on emitted file content rather than running a full `cargo check` of the generated
/// Tauri crate — that needs the Tauri toolchain, which is not guaranteed in CI. The
/// string assertion is the deterministic guard.
// TODO(ci): cargo check the generated Tauri crate when the tauri toolchain is available.
#[test]
fn tauri_desktop_target_wires_scheduled_jobs() {
    init_vox_binary_once();
    let src = r#"
@scheduled("1m")
fn heartbeat() { }
"#;
    let res = vox_compiler::pipeline::run_frontend_str(src, "scheduled_desktop.vox")
        .expect("frontend ok");
    let module = res.hir;
    let out = vox_codegen::codegen_rust::generate(
        &module,
        "pkg",
        vox_codegen::codegen_rust::RustAppShell::TauriApp,
    )
    .expect("tauri codegen");
    let main = out
        .files
        .get("src-tauri/src/main.rs")
        .expect("desktop target must emit src-tauri/src/main.rs");
    assert!(
        main.contains("vox_workflow_runtime::scheduled::register"),
        "desktop main.rs must register @scheduled fns:\n{main}"
    );
    assert!(
        main.contains("scheduled::start"),
        "desktop main.rs must start the scheduler:\n{main}"
    );
    assert!(
        main.contains("load_hir_module_from_embedded"),
        "desktop main.rs must embed + register the HirModule:\n{main}"
    );
}

/// Regression gate: `mobile.ts` must import from the `@vox/runtime` adapter contract,
/// never from `@tauri-apps/api/event` directly. Catches any future split-brain attempt
/// to wire mobile primitives straight to Tauri (which would break the RN target).
#[test]
fn mobile_emit_uses_adapter_contract_not_direct_tauri() {
    init_vox_binary_once();
    let run = BuildRun::run("mobile_back_button");
    run.assert_success();
    let mobile_ts = std::fs::read_to_string(run.out_dir.path().join("mobile.ts"))
        .expect("mobile.ts present after vox build");
    assert!(
        mobile_ts.contains("@vox/runtime"),
        "mobile.ts must import from `@vox/runtime`; got:\n{mobile_ts}"
    );
    assert!(
        !mobile_ts.contains("@tauri-apps/api/event"),
        "mobile.ts must not import from `@tauri-apps/api/event` directly — the runtime adapter \
         wraps Tauri events. Found split-brain emit:\n{mobile_ts}"
    );
    assert!(
        !mobile_ts.contains("@tauri-apps/api/core"),
        "mobile.ts must not import from `@tauri-apps/api/core` directly — the runtime adapter \
         wraps Tauri invoke calls. Found split-brain emit:\n{mobile_ts}"
    );
    assert!(
        mobile_ts.contains("voxRuntime.onBackButton"),
        "mobile.ts must call `voxRuntime.onBackButton`; got:\n{mobile_ts}"
    );
}

/// `@query` + `@mutation` must emit `vox-client.ts` + `openapi.json` + contract JSON.
#[test]
fn build_endpoint_query_mutation_succeeds_end_to_end() {
    init_vox_binary_once();
    let run = BuildRun::run("endpoint_query_mutation");
    run.assert_success();
    run.assert_no_panic();
    run.assert_expected_files();
}

/// `state_machine` declarations must emit a discriminated-union + reducer TypeScript file.
#[test]
fn build_state_machine_succeeds_end_to_end() {
    init_vox_binary_once();
    let run = BuildRun::run("state_machine");
    run.assert_success();
    run.assert_no_panic();
    run.assert_expected_files();
}

/// Cross-cutting fixture exercising orchestration that smaller fixtures don't:
/// components + state + route + form + endpoint + state machine all in one file.
#[test]
fn build_full_app_succeeds_end_to_end() {
    init_vox_binary_once();
    let run = BuildRun::run("full_app");
    run.assert_success();
    run.assert_no_panic();
    run.assert_expected_files();
}

/// Data-driven check: every directory under `tests/fixtures/` containing a `main.vox`
/// must pass the fast-path gates. Adding a new fixture only requires creating the
/// directory + files; no test code change.
///
/// Run the heavier `assert_all` (`tsc` + `cargo check`) per-fixture via dedicated
/// tests above when wanting to catch emitter-output-quality regressions; this one
/// is the broad regression gate.
#[test]
fn build_every_fixture_passes_fast_path() {
    let fixtures_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");
    let mut names: Vec<String> = std::fs::read_dir(&fixtures_root)
        .expect("read fixtures dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter(|e| e.path().join("main.vox").is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert!(
        !names.is_empty(),
        "no fixtures discovered under {}",
        fixtures_root.display()
    );

    let mut failures: Vec<String> = Vec::new();
    for name in &names {
        init_vox_binary_once();
        let run = BuildRun::run(name);
        // Negative fixtures are deliberate build failures (e.g. a contrast
        // violation); a dedicated test asserts their specific failure mode.
        if run.expected.expect_failure {
            continue;
        }
        if !run.success {
            failures.push(format!(
                "{name}: non-zero exit\nstdout:\n{}\nstderr:\n{}",
                run.stdout, run.stderr
            ));
            continue;
        }
        if run.stderr.contains("panicked at") {
            failures.push(format!(
                "{name}: stderr contains `panicked at`\n{}",
                run.stderr
            ));
            continue;
        }
        for required in &run.expected.required {
            let p = run.out_dir.path().join(required);
            if !p.is_file() {
                failures.push(format!("{name}: missing required file {required}"));
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "{} fixture(s) failed end-to-end build:\n{}",
            failures.len(),
            failures.join("\n---\n")
        );
    }
    eprintln!("vox-cli-tests: {} fixture(s) passed", names.len());
}

/// The contrast/occlusion guarantees follow the view tree, not the target: a
/// gray-on-white contrast violation must fail `vox build --target mobile`, the
/// same way it fails the web build. Regression gate for audit gap XP-4/CONTRAST-5.
#[test]
fn build_mobile_fails_on_contrast_violation() {
    init_vox_binary_once();
    let run = BuildRun::run("mobile_bad_contrast");
    assert!(
        !run.success,
        "mobile build must gate on the web_ir validators.\n--- stdout ---\n{}\n--- stderr ---\n{}",
        run.stdout, run.stderr
    );
    assert!(
        run.stderr.contains("insufficient_contrast"),
        "expected insufficient_contrast in stderr, got:\n{}",
        run.stderr
    );
}

/// B4: the `modal` tier primitive now lowers to a react-native `<Modal>` instead of
/// hard-erroring. The build succeeds and the emitted component uses `<Modal>`.
/// Regression gate for audit gap XP-2 (overlay-family RN representation).
#[test]
fn build_mobile_modal_emits_react_native_modal() {
    init_vox_binary_once();
    let run = BuildRun::run("mobile_modal_unsupported");
    run.assert_success();
    run.assert_no_panic();
    let home = std::fs::read_to_string(run.out_dir.path().join("Home.tsx")).expect("Home.tsx");
    assert!(
        home.contains("<Modal"),
        "modal must lower to react-native <Modal>; got:\n{home}"
    );
    assert!(
        home.contains("Modal,") && home.contains("from \"react-native\""),
        "Modal must be imported from react-native; got:\n{home}"
    );
}
