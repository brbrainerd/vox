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
    let todo_tsx = std::fs::read_to_string(run.out_dir.path().join("TodoList.tsx"))
        .expect("TodoList.tsx");
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
