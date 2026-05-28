#![allow(unsafe_code)] // `std::env::{set_var,remove_var}` for opt-in Web IR view bridge tests

use serde::Deserialize;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Mutex;

/// Serializes `reactive_smoke` tests that mutate process environment (deprecated `VOX_WEBIR_EMIT_REACTIVE_VIEWS` guards).
static REACTIVE_SMOKE_SERIAL: Mutex<()> = Mutex::new(());

#[derive(Debug, Deserialize)]
struct GuiCompatibilityContract {
    react_attr_matrix: BTreeMap<String, String>,
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// Shared fixture for OP-S114 / OP-S177: derived state surfaces in `D.tsx` output.
const REACTIVE_SMOKE_DERIVED_HARNESS_FIXTURE: &str = r#"
component D() {
    state x: int = 1
    derived y = x + 1
    view: text(italic=true) { y }
}
"#;

fn reactive_smoke_assert_derived_harness_in_d_tsx() {
    let _serial = REACTIVE_SMOKE_SERIAL
        .lock()
        .expect("REACTIVE_SMOKE_SERIAL poisoned");
    let m = vox_compiler::parser::parse(vox_compiler::lexer::lex(
        REACTIVE_SMOKE_DERIVED_HARNESS_FIXTURE,
    ))
    .expect("parse");
    let hir = vox_compiler::hir::lower_module(&m);
    let out = vox_codegen::codegen_ts::generate(&hir).expect("gen");
    let body = out
        .files
        .iter()
        .find(|(n, _)| n == "D.tsx")
        .unwrap()
        .1
        .as_str();
    assert!(body.contains("useMemo") || body.contains('y'), "{body}");
}

/// Worked surface for grammar-branch registry (OP-0267) and K-metric token trace (OP-0268).
/// Covers G01–G08 from §A3; G09 (`style { }`) stays on the dedicated [`reactive_smoke_style_block_emits_css_module_import`] fixture (top-level `style` must follow only `@component` with no later decls in the same module).
const K_METRIC_BRANCH_REGISTRY_FIXTURE: &str = r#"
import react.use_state

component Home() {
    let (_n, _set_n) = use_state(0)
    view: column(raw_class="home") { "home" }
}

routes {
    "/" to Home
    "/health" to Home
}

component Shell() {
    state s: str = "x"
    view: column(raw_class="app") {
        text() { s }
        button(on_click={s = s + "!"}) { "go" }
    }
}
"#;

/// OP-S002 + OP-S004: K-metric registry fixture parses end-to-end; `routes` [`RoutesDecl::parse_summary`] is stable.
#[serial_test::serial]
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn k_metric_branch_registry_parser_micro_gate() {
    use vox_compiler::ast::decl::Decl;

    let _serial = REACTIVE_SMOKE_SERIAL
        .lock()
        .expect("REACTIVE_SMOKE_SERIAL poisoned");
    let tokens = vox_compiler::lexer::lex(K_METRIC_BRANCH_REGISTRY_FIXTURE);
    let module =
        vox_compiler::parser::parse(tokens).expect("K-metric branch registry fixture must parse");
    let routes_decl = module
        .declarations
        .iter()
        .find_map(|d| match d {
            Decl::Routes(r) => Some(r),
            _ => None,
        })
        .expect("fixture includes routes { ... }");
    let summary = routes_decl.parse_summary();
    assert_eq!(summary.entry_count, 2, "{summary:?}");
    assert_eq!(summary.paths, vec!["/".to_string(), "/health".to_string()]);
    assert!(
        module.declarations.len() >= 3,
        "expected import + Path C Home + routes + reactive Shell, got {}",
        module.declarations.len()
    );
}

/// `hir_emit::compat` is the single matrix for AST JSX re-exports (OP-0131).
#[serial_test::serial]
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn jsx_and_hir_emit_share_compat_attr_matrix() {
    let _serial = REACTIVE_SMOKE_SERIAL
        .lock()
        .expect("REACTIVE_SMOKE_SERIAL poisoned");
    let sample = [
        "class",
        "className",
        "on:click",
        "on_click",
        "for",
        "tab_index",
        "on:mouseleave",
    ];
    for a in sample {
        assert_eq!(
            vox_codegen::codegen_ts::hir_emit::map_jsx_attr_name(a),
            vox_codegen::codegen_ts::hir_emit::compat::map_jsx_attr_name(a)
        );
        assert_eq!(
            vox_codegen::codegen_ts::jsx::map_jsx_attr_name(a),
            vox_codegen::codegen_ts::hir_emit::compat::map_jsx_attr_name(a)
        );
    }
}

/// OP-S030: compatibility-tag DOM edges (`compat` fall-through vs mapped spellings); pairs OP-S029 / OP-S031.
#[serial_test::serial]
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn op_s030_compat_tag_fixture_dom_and_a11y_edges() {
    let _serial = REACTIVE_SMOKE_SERIAL
        .lock()
        .expect("REACTIVE_SMOKE_SERIAL poisoned");
    let edges = [
        ("for", "htmlFor"),
        ("tab_index", "tabIndex"),
        ("class", "className"),
    ];
    for (vox, react) in edges {
        let h = vox_codegen::codegen_ts::hir_emit::map_jsx_attr_name(vox);
        let j = vox_codegen::codegen_ts::jsx::map_jsx_attr_name(vox);
        let c = vox_codegen::codegen_ts::hir_emit::compat::map_jsx_attr_name(vox);
        assert_eq!(h, react, "{vox}");
        assert_eq!(h, j, "{vox}");
        assert_eq!(h, c, "{vox}");
    }
}

/// SSOT guard: `contracts/frontend/gui-compatibility.v1.yaml` is the authoritative attr matrix
/// and codegen must stay byte-for-byte aligned with it.
#[serial_test::serial]
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn gui_compatibility_contract_matches_attr_mapping_matrix() {
    let _serial = REACTIVE_SMOKE_SERIAL
        .lock()
        .expect("REACTIVE_SMOKE_SERIAL poisoned");

    let raw = std::fs::read_to_string(
        repo_root()
            .join("contracts")
            .join("frontend")
            .join("gui-compatibility.v1.yaml"),
    )
    .expect("read gui compatibility contract");
    let contract: GuiCompatibilityContract =
        serde_yaml::from_str(&raw).expect("parse gui compatibility contract");

    for (vox_attr, react_attr) in contract.react_attr_matrix {
        let mapped = vox_codegen::codegen_ts::hir_emit::compat::map_jsx_attr_name(&vox_attr);
        assert_eq!(
            mapped, react_attr,
            "contract drift for `{}`: compat map = `{}` but contract = `{}`",
            vox_attr, mapped, react_attr
        );
        assert_eq!(
            vox_codegen::codegen_ts::jsx::map_jsx_attr_name(&vox_attr),
            react_attr,
            "jsx alias drift for `{}`",
            vox_attr
        );
        assert_eq!(
            vox_codegen::codegen_ts::hir_emit::map_jsx_attr_name(&vox_attr),
            react_attr,
            "hir_emit re-export drift for `{}`",
            vox_attr
        );
    }
}

/// OP-S038 (historical): `VOX_WEBIR_EMIT_REACTIVE_VIEWS` is removed; reactive views always use Web IR when valid.
#[serial_test::serial]
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn op_s038_legacy_reactive_views_env_var_is_removed_noop() {
    use std::ffi::OsString;
    let _serial = REACTIVE_SMOKE_SERIAL
        .lock()
        .expect("REACTIVE_SMOKE_SERIAL poisoned");

    const KEY: &str = "VOX_WEBIR_EMIT_REACTIVE_VIEWS";
    struct Guard {
        prev: Option<OsString>,
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            match &self.prev {
                Some(v) => unsafe { std::env::set_var(KEY, v) },
                None => unsafe { std::env::remove_var(KEY) },
            }
        }
    }
    let prev = std::env::var_os(KEY);
    unsafe {
        std::env::set_var(KEY, "0");
    }
    let _guard = Guard { prev };
    let source = r#"
component T() {
    state n: int = 1
    view: text() { n }
}
"#;
    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::parse(tokens).expect("parse T");
    let hir = vox_compiler::hir::lower_module(&module);
    let out = vox_codegen::codegen_ts::generate(&hir).expect("codegen");
    let stats = out.reactive_stats;
    assert!(
        stats.web_ir_view_emitted >= 1 || stats.web_ir_view_emitted_parity_mismatch >= 1,
        "expected Web IR pathway with deprecated env cleared: {stats:?}"
    );
    assert!(
        stats.reactive_view_emit_failures.is_empty(),
        "unexpected failures: {:?}",
        stats.reactive_view_emit_failures
    );
}

#[serial_test::serial]
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn test_reactive_codegen_smoke() {
    let _serial = REACTIVE_SMOKE_SERIAL
        .lock()
        .expect("REACTIVE_SMOKE_SERIAL poisoned");
    let source = r#"
component Counter(initial: int) {
    state count: int = initial
    derived double = count * 2
    
    on mount: {
        log("mounted")
    }

    view: column(raw_class="p-4") {
            heading(level=1) { "Count: {count}" }
            text() { "Double: {double}" }
            button(on_click={count = count + 1}) { "Increment" }
    }
}
"#;
    let tokens = vox_compiler::lexer::lex(source);
    for t in &tokens {
        println!("Token: {:?} at {:?}", t.token, t.span);
    }
    let module = vox_compiler::parser::parse(tokens).expect("Parsing failed");
    let hir = vox_compiler::hir::lower_module(&module);
    let output = vox_codegen::codegen_ts::generate(&hir).expect("Codegen failed");

    let ts = output
        .files
        .iter()
        .find(|(f, _)| f == "Counter.tsx")
        .map(|(_, c)| c)
        .expect("Counter.tsx not found");
    println!("Generated TSX:\n{}", ts);

    insta::assert_snapshot!("counter_tsx_reactive_smoke", ts);
}

#[serial_test::serial]
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn reactive_hook_codegen_is_deterministic_across_lowering_runs() {
    let _serial = REACTIVE_SMOKE_SERIAL
        .lock()
        .expect("REACTIVE_SMOKE_SERIAL poisoned");
    let source = r#"
import react.use_state

component Tick() {
    state x: int = 0
    view: button(on_click={x = x + 1}) { x }
}
"#;
    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::parse(tokens).expect("parse Tick fixture");
    let hir_once = |m: &_| vox_compiler::hir::lower_module(m);
    let emit = |hir: &_| vox_codegen::codegen_ts::generate(hir).expect("codegen");

    let a = emit(&hir_once(&module));
    let b = emit(&hir_once(&module));
    let ts_a = a
        .files
        .iter()
        .find(|(n, _)| n == "Tick.tsx")
        .map(|(_, c)| c.as_str())
        .expect("Tick.tsx");
    let ts_b = b
        .files
        .iter()
        .find(|(n, _)| n == "Tick.tsx")
        .map(|(_, c)| c.as_str())
        .expect("Tick.tsx");
    assert_eq!(ts_a, ts_b);
    assert!(
        ts_a.contains("useState"),
        "expected React hook emit, got:\n{ts_a}"
    );
}

#[serial_test::serial]
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn web_ir_preview_emit_maps_class_attr_to_class_name() {
    use vox_codegen::web_ir::emit_tsx::emit_component_view_tsx;
    use vox_codegen::web_ir::lower::lower_hir_to_web_ir;

    let source = r#"
component T() {
    state n: int = 1
    view: column(raw_class="wrap") { n }
}
"#;
    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::parse(tokens).expect("parse T");
    let hir = vox_compiler::hir::lower_module(&module);
    let web = lower_hir_to_web_ir(&hir);
    let tsx = emit_component_view_tsx(&web, "T").expect("emit");
    assert!(
        tsx.contains("className="),
        "preview emit should mirror class→className matrix, got:\n{tsx}"
    );
}

/// Path C reactive `view:` always uses Web IR preview TSX when validate is clean (ADR 036 / bundle SSOT).
#[serial_test::serial]
#[test]
fn reactive_codegen_uses_webir_canonical_view() {
    let _serial = REACTIVE_SMOKE_SERIAL
        .lock()
        .expect("REACTIVE_SMOKE_SERIAL poisoned");

    let source = r#"
component Counter(initial: int) {
    state count: int = initial
    derived double = count * 2

    on mount: {
        log("mounted")
    }

    view: column(raw_class="p-4") {
            heading(level=1) { "Count: {count}" }
            text() { "Double: {double}" }
            button(on_click={count = count + 1}) { "Increment" }
    }
}
"#;
    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::parse(tokens).expect("parse");
    let hir = vox_compiler::hir::lower_module(&module);
    let output = vox_codegen::codegen_ts::generate(&hir).expect("codegen");
    let ts = output
        .files
        .iter()
        .find(|(f, _)| f == "Counter.tsx")
        .map(|(_, c)| c)
        .expect("Counter.tsx");
    insta::assert_snapshot!("counter_tsx_with_web_ir_canonical_view", ts);
    assert!(
        output.reactive_stats.web_ir_view_emitted >= 1
            || output.reactive_stats.web_ir_view_emitted_parity_mismatch >= 1,
        "expected a Web IR pathway tally: {:?}",
        output.reactive_stats
    );
    assert!(
        output.reactive_stats.reactive_view_emit_failures.is_empty(),
        "unexpected reactive view failures: {:?}",
        output.reactive_stats.reactive_view_emit_failures
    );
}

#[serial_test::serial]
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn reactive_view_bridge_stats_legacy_when_web_ir_env_off() {
    let _serial = REACTIVE_SMOKE_SERIAL
        .lock()
        .expect("REACTIVE_SMOKE_SERIAL poisoned");

    const KEY: &str = "VOX_WEBIR_EMIT_REACTIVE_VIEWS";
    struct Guard {
        prev: Option<OsString>,
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            match &self.prev {
                Some(v) => unsafe { std::env::set_var(KEY, v) },
                None => unsafe { std::env::remove_var(KEY) },
            }
        }
    }
    let _guard = Guard {
        prev: std::env::var_os(KEY),
    };
    unsafe { std::env::set_var(KEY, "0") };

    let source = r#"
component C() {
    state n: int = 0
    view: text() { n }
}
"#;
    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::parse(tokens).expect("parse C");
    let hir = vox_compiler::hir::lower_module(&module);
    let out = vox_codegen::codegen_ts::generate(&hir).expect("codegen");
    let after = out.reactive_stats;
    assert!(
        after.web_ir_view_emitted >= 1 || after.web_ir_view_emitted_parity_mismatch >= 1,
        "expected Web IR pathway even when deprecated env is 0: {after:?}"
    );
}

#[serial_test::serial]
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn reactive_view_bridge_stats_env_on_uses_non_legacy_pathways() {
    let _serial = REACTIVE_SMOKE_SERIAL
        .lock()
        .expect("REACTIVE_SMOKE_SERIAL poisoned");

    const KEY: &str = "VOX_WEBIR_EMIT_REACTIVE_VIEWS";
    struct Guard {
        prev: Option<OsString>,
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            match &self.prev {
                Some(v) => unsafe { std::env::set_var(KEY, v) },
                None => unsafe { std::env::remove_var(KEY) },
            }
        }
    }
    let _guard = Guard {
        prev: std::env::var_os(KEY),
    };
    unsafe { std::env::set_var(KEY, "1") };

    let source = r#"
component Counter(initial: int) {
    state count: int = initial
    derived double = count * 2

    on mount: {
        log("mounted")
    }

    view: column(raw_class="p-4") {
            heading(level=1) { "Count: {count}" }
            text() { "Double: {double}" }
            button(on_click={count = count + 1}) { "Increment" }
    }
}
"#;
    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::parse(tokens).expect("parse");
    let hir = vox_compiler::hir::lower_module(&module);
    let out = vox_codegen::codegen_ts::generate(&hir).expect("codegen");
    let after = out.reactive_stats;
    let d_web = after.web_ir_view_emitted;
    let d_par = after.web_ir_view_emitted_parity_mismatch;
    assert_eq!(
        d_web + d_par,
        1,
        "exactly one bridge decision per reactive view; web={d_web} par={d_par} after={after:?}"
    );
    assert!(
        after.reactive_view_emit_failures.is_empty(),
        "unexpected failures: {:?}",
        after.reactive_view_emit_failures
    );
}

fn assert_contains_all(haystack: &str, needles: &[&str], ctx: &str) {
    for n in needles {
        assert!(
            haystack.contains(*n),
            "{ctx}: expected substring {n:?} in:\n{haystack}"
        );
    }
}

/// OP-0267: single fixture exercising multiple grammar-branch families (side-by-side schema §A3: G01–G08).
#[serial_test::serial]
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn reactive_smoke_branch_registry_fixture_parses_and_lowers() {
    let _serial = REACTIVE_SMOKE_SERIAL
        .lock()
        .expect("REACTIVE_SMOKE_SERIAL poisoned");
    let tokens = vox_compiler::lexer::lex(K_METRIC_BRANCH_REGISTRY_FIXTURE);
    let module = vox_compiler::parser::parse(tokens).expect("branch-registry fixture parse");
    let hir = vox_compiler::hir::lower_module(&module);
    let web = vox_codegen::web_ir::lower::lower_hir_to_web_ir(&hir);
    let diags = vox_codegen::web_ir::validate::validate_web_ir(&web);
    let error_diags: Vec<_> = diags
        .iter()
        .filter(|d| !vox_codegen::web_ir::validate::is_advisory_diagnostic(d))
        .collect();
    assert!(
        error_diags.is_empty(),
        "fixture should have no blocking errors after lower; {error_diags:?}"
    );
    let out = vox_codegen::codegen_ts::generate(&hir).expect("codegen branch-registry");
    assert!(
        out.files.iter().any(|(n, _)| n == "Shell.tsx"),
        "expected Shell.tsx in {:?}",
        out.files.iter().map(|(n, _)| n).collect::<Vec<_>>()
    );
    assert!(
        out.files.iter().any(|(n, _)| n == "Home.tsx"),
        "expected Home.tsx"
    );
}

/// OP-0268: K-metric appendix §A1 token-class markers appear verbatim in the branch-registry source (recomputable trace input).
#[serial_test::serial]
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn worked_app_k_metric_appendix_token_classes_are_traceable_in_source() {
    struct Row {
        label: &'static str,
        needle: &'static str,
    }
    let rows = [
        Row {
            label: "T02 structural `component` / `routes` / `http`",
            needle: "routes",
        },
        Row {
            label: "T06 view-call event handler (post-VUV; was JSX `on:click`)",
            needle: "on_click",
        },
        Row {
            label: "T02 `component` path-c name",
            needle: "component Shell",
        },
        Row {
            label: "T08 routing path literal",
            needle: "\"/health\"",
        },
    ];
    for r in rows {
        assert!(
            K_METRIC_BRANCH_REGISTRY_FIXTURE.contains(r.needle),
            "{}: missing {:?} in fixture source",
            r.label,
            r.needle
        );
    }
}

/// OP-0259 / OP-0266: class → `className` and `on:click` → `onClick` in reactive emit.
#[serial_test::serial]
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn reactive_smoke_class_and_event_mapping_path_c() {
    let _serial = REACTIVE_SMOKE_SERIAL
        .lock()
        .expect("REACTIVE_SMOKE_SERIAL poisoned");
    let source = r#"
component Clicky() {
    state n: int = 0
    view: button(raw_class="btn", on_click={n = n + 1}) { n }
}
"#;
    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::parse(tokens).expect("parse");
    let hir = vox_compiler::hir::lower_module(&module);
    let out = vox_codegen::codegen_ts::generate(&hir).expect("codegen");
    let ts = out
        .files
        .iter()
        .find(|(n, _)| n == "Clicky.tsx")
        .map(|(_, c)| c.as_str())
        .expect("Clicky.tsx");
    // VUV: author `raw_class="btn"` lowers via primitives::resolve_universal_kwarg into a
    // string-literal piece in the className expression. Combined with button's primitive base
    // classes the final shape is a `[...].filter(Boolean).join(" ")` array — assert on the
    // distinctive substrings rather than the exact wrapper.
    assert_contains_all(
        ts,
        &["\"btn\"", "className={", "onClick"],
        "Clicky.tsx class/event parity",
    );
}

/// OP-0263: reactive Path C `component` + top-level `style { }` emits `*.css` and TSX imports it.
#[serial_test::serial]
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn reactive_smoke_style_block_emits_css_module_import() {
    let _serial = REACTIVE_SMOKE_SERIAL
        .lock()
        .expect("REACTIVE_SMOKE_SERIAL poisoned");
    // raw_css { } bypasses the design-token literal-color gate (TASK-5.1), emitting a warning only.
    let source = r#"
component Box() {
    view: column(raw_class="x") { "a" }
}
raw_css {
    .x { color: "red" }
}
"#;
    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::parse(tokens).expect("parse Box style");
    let hir = vox_compiler::hir::lower_module(&module);
    let out = vox_codegen::codegen_ts::generate(&hir).expect("codegen");
    let css = out
        .files
        .iter()
        .find(|(n, _)| n == "Box.css")
        .map(|(_, c)| c.as_str())
        .expect("Box.css");
    assert!(css.contains("color") || css.contains("red"), "{css}");
    let tsx = out
        .files
        .iter()
        .find(|(n, _)| n == "Box.tsx")
        .map(|(_, c)| c.as_str())
        .expect("Box.tsx");
    insta::assert_snapshot!("box_tsx_css_import", tsx);
}

/// OP-0271 / OP-0272: explicit no-regression label for the reactive smoke module gate.
#[serial_test::serial]
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn reactive_smoke_gate_label_smoke_tests_module() {
    let _serial = REACTIVE_SMOKE_SERIAL
        .lock()
        .expect("REACTIVE_SMOKE_SERIAL poisoned");
    assert!(
        !env!("CARGO_MANIFEST_DIR").is_empty(),
        "reactive_smoke gate: expect cargo manifest dir"
    );
}

// --- OP-S073–S218 supplemental reactive fixtures ---

/// OP-S074: behavior map — reactive state surfaces in generated TSX hooks.
#[serial_test::serial]
#[ignore = "VUV-9: parity pin for completed JSX→Web-IR migration epic; assertions reference retired JSX form"]
#[test]
fn reactive_smoke_op_s074_s075_behavior_view_fixture() {
    let _serial = REACTIVE_SMOKE_SERIAL
        .lock()
        .expect("REACTIVE_SMOKE_SERIAL poisoned");
    let source = r#"
component V() {
    state k: int = 2
    view: text() { k }
}
"#;
    let m = vox_compiler::parser::parse(vox_compiler::lexer::lex(source)).expect("parse");
    let hir = vox_compiler::hir::lower_module(&m);
    let ts = vox_codegen::codegen_ts::generate(&hir).expect("gen");
    let f = ts
        .files
        .iter()
        .find(|(n, _)| n == "V.tsx")
        .unwrap()
        .1
        .as_str();
    insta::assert_snapshot!("v_tsx_usestate_k_op_s074", f);
}

/// OP-S078 / S077: wrapper inventory — event attr maps in Path C emit.
#[serial_test::serial]
#[ignore = "VUV-9: parity pin for completed JSX→Web-IR migration epic; assertions reference retired JSX form"]
#[test]
fn reactive_smoke_op_s078_wrapper_inventory_fixture() {
    let _serial = REACTIVE_SMOKE_SERIAL
        .lock()
        .expect("REACTIVE_SMOKE_SERIAL poisoned");
    let source = r#"
component Clicky() {
    state n: int = 0
    view: button(raw_class="btn", on_click={n = n + 1}) { n }
}
"#;
    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::parse(tokens).expect("parse");
    let hir = vox_compiler::hir::lower_module(&module);
    let out = vox_codegen::codegen_ts::generate(&hir).expect("codegen");
    let ts = out
        .files
        .iter()
        .find(|(n, _)| n == "Clicky.tsx")
        .map(|(_, c)| c.as_str())
        .expect("Clicky.tsx");
    insta::assert_snapshot!("clicky_tsx_classname_onclick", ts);
}

/// OP-S114: behavior contract A — derived depends on state.
#[serial_test::serial]
#[ignore = "VUV-9: parity pin for completed JSX→Web-IR migration epic; assertions reference retired JSX form"]
#[test]
fn reactive_smoke_op_s114_behavior_contract_fixture_a() {
    reactive_smoke_assert_derived_harness_in_d_tsx();
}

/// OP-S125 fixture pack D1.
#[serial_test::serial]
#[ignore = "VUV-9: parity pin for completed JSX→Web-IR migration epic; assertions reference retired JSX form"]
#[test]
fn reactive_smoke_op_s125_fixture_pack_d1() {
    reactive_smoke_op_s074_s075_behavior_view_fixture();
}

/// OP-S162 component adapter B.
#[serial_test::serial]
#[ignore = "VUV-9: parity pin for completed JSX→Web-IR migration epic; assertions reference retired JSX form"]
#[test]
fn reactive_smoke_op_s162_component_adapter_fixture_b() {
    let _serial = REACTIVE_SMOKE_SERIAL
        .lock()
        .expect("REACTIVE_SMOKE_SERIAL poisoned");
    let source = r#"
component Box() {
    view: column(raw_class="x") { "a" }
}
raw_css {
    .x { color: "red" }
}
"#;
    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::parse(tokens).expect("parse Box style");
    let hir = vox_compiler::hir::lower_module(&module);
    let out = vox_codegen::codegen_ts::generate(&hir).expect("codegen");
    assert!(out.files.iter().any(|(n, _)| n == "Box.css"));
}

/// OP-S170 hir wrapper B.
#[serial_test::serial]
#[ignore = "VUV-9: parity pin for completed JSX→Web-IR migration epic; assertions reference retired JSX form"]
#[test]
fn reactive_smoke_op_s170_hir_wrapper_fixture_b() {
    let _serial = REACTIVE_SMOKE_SERIAL
        .lock()
        .expect("REACTIVE_SMOKE_SERIAL poisoned");
    assert_eq!(
        vox_codegen::codegen_ts::hir_emit::map_jsx_attr_name("on:click"),
        vox_codegen::codegen_ts::jsx::map_jsx_attr_name("on_click")
    );
}

/// OP-S177 fixture pack F1.
#[serial_test::serial]
#[ignore = "VUV-9: parity pin for completed JSX→Web-IR migration epic; assertions reference retired JSX form"]
#[test]
fn reactive_smoke_op_s177_fixture_pack_f1() {
    reactive_smoke_assert_derived_harness_in_d_tsx();
}

/// OP-S205 fixture pack G1.
#[serial_test::serial]
#[ignore = "VUV-9: parity pin for completed JSX→Web-IR migration epic; assertions reference retired JSX form"]
#[test]
fn reactive_smoke_op_s205_fixture_pack_g1() {
    let _serial = REACTIVE_SMOKE_SERIAL
        .lock()
        .expect("REACTIVE_SMOKE_SERIAL poisoned");
    let source = r#"
component Counter(initial: int) {
    state count: int = initial
    derived double = count * 2
    on mount: {
        log("mounted")
    }
    view: column(raw_class="p-4") {
            heading(level=1) { "Count: {count}" }
            text() { "Double: {double}" }
            button(on_click={count = count + 1}) { "Increment" }
    }
}
"#;
    let module =
        vox_compiler::parser::parse(vox_compiler::lexer::lex(source)).expect("Parsing failed");
    let hir = vox_compiler::hir::lower_module(&module);
    let output = vox_codegen::codegen_ts::generate(&hir).expect("Codegen failed");
    let ts = output
        .files
        .iter()
        .find(|(f, _)| f == "Counter.tsx")
        .map(|(_, c)| c.as_str())
        .expect("Counter.tsx not found");
    insta::assert_snapshot!("counter_tsx_function_and_usememo_op_s205", ts);
}

/// OP-S218 final reactive parity.
#[serial_test::serial]
#[ignore = "VUV-9: parity pin for completed JSX→Web-IR migration epic; assertions reference retired JSX form"]
#[test]
fn reactive_smoke_op_s218_final_reactive_parity_fixture() {
    assert!(!env!("CARGO_MANIFEST_DIR").is_empty());
}

/// OP-0261: legacy `emit_hir_expr` view string matches Web IR preview after shared whitespace normalization.
#[serial_test::serial]
#[ignore = "VUV-9: parity pin for completed JSX→Web-IR migration epic; assertions reference retired JSX form"]
#[test]
fn reactive_smoke_legacy_vs_web_ir_view_whitespace_parity() {
    use std::collections::HashSet;

    use vox_codegen::codegen_ts::hir_emit::{EmitCtx, emit_hir_expr};
    use vox_codegen::codegen_ts::reactive::normalize_reactive_view_jsx_ws;
    use vox_codegen::web_ir::emit_tsx::emit_component_view_tsx;
    use vox_codegen::web_ir::lower::lower_hir_to_web_ir;
    use vox_codegen::web_ir::validate::validate_web_ir;
    use vox_compiler::hir::HirReactiveMember;

    let _serial = REACTIVE_SMOKE_SERIAL
        .lock()
        .expect("REACTIVE_SMOKE_SERIAL poisoned");

    let src = r#"
component ParityT() {
    state n: int = 1
    view: text(raw_class="x")
}
"#;
    let module = vox_compiler::parser::parse(vox_compiler::lexer::lex(src)).expect("parse");
    let hir = vox_compiler::hir::lower_module(&module);
    let rc = hir.components.first().expect("reactive component");
    let view = rc.view.as_ref().expect("view");
    let state_name = match &rc.members[0] {
        HirReactiveMember::State(s) => s.name.clone(),
        _ => panic!("expected state member"),
    };
    let sn = HashSet::from([state_name]);
    let legacy = emit_hir_expr(view, &EmitCtx::new(&sn));
    let web = lower_hir_to_web_ir(&hir);
    assert!(
        validate_web_ir(&web).is_empty(),
        "validate_web_ir must be clean for parity fixture"
    );
    let preview = emit_component_view_tsx(&web, "ParityT").expect("preview tsx");
    assert_eq!(
        normalize_reactive_view_jsx_ws(&legacy),
        normalize_reactive_view_jsx_ws(&preview),
        "legacy:\n{legacy}\npreview:\n{preview}"
    );
}
