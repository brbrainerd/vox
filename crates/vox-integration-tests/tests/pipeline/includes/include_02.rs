// --- New golden example integration tests (T031–T043) ---

// generics_option.vox
const GENERICS_OPTION_SRC: &str = r#"type Option =
    | Some(value: str)
    | None

type Result =
    | Ok(value: str)
    | Err(message: str)

fn unwrap_or(opt: Option, default: str) to str {
    match opt {
        Some(value) => value
        None => default
    }
}

@test fn test_unwrap_some() to Unit {
    let opt = Some("hello")
    let result = unwrap_or(opt, "default")
    assert(result is "hello")
}

@test fn test_ok_result() to Unit {
    let r = Ok("success")
    match r {
        Ok(value) => assert(value is "success")
        Err(msg) => assert(false)
    }
}
"#;

#[test]
fn pipeline_generics_option_parse() {
    let tokens = lex(GENERICS_OPTION_SRC);
    let module = parse(tokens).expect("generics_option should parse");
    // type Option + type Result + fn unwrap_or + @test x2
    assert_eq!(module.declarations.len(), 5, "2 types + 1 fn + 2 tests");
}

#[test]
fn pipeline_generics_option_codegen() {
    let tokens = lex(GENERICS_OPTION_SRC);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);
    let output = generate(&hir).unwrap();
    let types = output.files.iter().find(|(n, _)| n == "types.ts").unwrap();
    insta::assert_snapshot!("generics_option_types_ts_emit", types.1);
}

// hooks_demo.vox
const HOOKS_DEMO_SRC: &str = r#"import react.use_state
import react.use_effect
import react.use_memo
import react.use_ref
import react.use_callback

component HooksDemo() {
    let (count, set_count) = use_state(0)
    let doubled = use_memo(fn(_x) count * 2)
    let input_ref = use_ref(0)
    let increment = use_callback(fn(_e) set_count(count + 1))
    use_effect(fn(_x) count)
    view: column(raw_class="hooks_demo") {
        column(raw_class="count-row") { count }
        column(raw_class="doubled-row") { doubled }
        button(raw_class="increment-btn", on_click=increment) { "+" }
    }
}

routes {
    "/" to HooksDemo
}
"#;

#[test]
fn pipeline_hooks_demo_parse() {
    let tokens = lex(HOOKS_DEMO_SRC);
    let module = parse(tokens).expect("hooks_demo should parse");
    // 5 imports + 1 component + 1 routes
    assert_eq!(module.declarations.len(), 7);
}

#[test]
fn pipeline_hooks_demo_codegen() {
    let tokens = lex(HOOKS_DEMO_SRC);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);
    let output = generate(&hir).unwrap();
    let tsx = output
        .files
        .iter()
        .find(|(n, _)| n == "HooksDemo.tsx")
        .unwrap();
    insta::assert_snapshot!("hooks_demo_tsx_all_hooks", tsx.1);
}

#[test]
fn pipeline_web_ir_preview_emit_hooks_reactive_fixture() {

    use vox_codegen::web_ir::emit_tsx::emit_component_view_tsx;
    use vox_codegen::web_ir::lower::lower_hir_to_web_ir;
    use vox_codegen::web_ir::validate::validate_web_ir;

    let tokens = lex(HOOKS_DEMO_SRC);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);
    let web = lower_hir_to_web_ir(&hir);
    assert!(
        validate_web_ir(&web).is_empty(),
        "hooks demo should validate clean as Web IR"
    );
    let preview = emit_component_view_tsx(&web, "HooksDemo").expect("preview HooksDemo view");
    assert!(
        preview.contains("hooks_demo") || preview.contains("\"hooks_demo\""),
        "expected hooks demo class/root in preview:\n{preview}"
    );

    // Mixed surface: Path C `Shell` (hooks) + Path C `Dash` — reactive `view:` uses Web IR preview TSX.
    let mix_tokens = lex(MIXED_SURFACE_SRC);
    let mix_mod = parse(mix_tokens).expect("parse MIXED_SURFACE");
    let mix_hir = vox_compiler::hir::lower_module(&mix_mod);
    // Do not nest `with_web_ir_validate_cleared` here: it also takes `ENV_MUTEX` and would deadlock.
    let _env_guard = ENV_MUTEX.lock().expect("ENV_MUTEX poisoned");

    let out = generate(&mix_hir).expect("MIXED_SURFACE codegen");
    let dash = out
        .files
        .iter()
        .find(|(n, _)| n == "Dash.tsx")
        .map(|(_, c)| c.as_str())
        .expect("Dash.tsx");
    assert!(
        dash.contains("Dash") || dash.contains("dashboard"),
        "Dash should reference its own component name or class:\n{dash}"
    );
    let shell = out
        .files
        .iter()
        .find(|(n, _)| n == "Shell.tsx")
        .map(|(_, c)| c.as_str())
        .expect("Shell.tsx");
    assert!(
        shell.contains("useState"),
        "Shell retains hooks:\n{shell}"
    );
    let stats = out.reactive_stats;
    assert!(
        stats.web_ir_view_emitted >= 1,
        "expected Web IR preview for reactive Dash; stats={stats:?}"
    );
}

// v0_component.vox
const V0_COMPONENT_SRC: &str = r#"@v0 "A modern analytics dashboard with KPI cards" Analytics {}

@v0 from "design/landing-mockup.png" LandingPage {}

routes {
    "/" to Analytics
    "/landing" to LandingPage
}
"#;

#[test]
fn pipeline_v0_parse() {
    let tokens = lex(V0_COMPONENT_SRC);
    let module = parse(tokens).expect("v0_component should parse");
    // 2 v0 + 1 routes
    assert_eq!(module.declarations.len(), 3);
}

#[test]
#[ignore = "@v0 components dropped from HIR (Path B removed); no TSX generated — owner: integration-tests sunset: 2026-12-31"]
fn pipeline_v0_codegen() {
    let tokens = lex(V0_COMPONENT_SRC);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);
    let output = generate(&hir).unwrap();
    let filenames: Vec<&str> = output.files.iter().map(|(n, _)| n.as_str()).collect();
    assert!(
        filenames.contains(&"Analytics.tsx"),
        "Should produce Analytics.tsx"
    );
    assert!(
        filenames.contains(&"LandingPage.tsx"),
        "Should produce LandingPage.tsx"
    );
    let analytics = output
        .files
        .iter()
        .find(|(n, _)| n == "Analytics.tsx")
        .unwrap();
    assert!(
        analytics.1.contains("@v0 generated"),
        "Analytics should be v0 placeholder"
    );
    let landing = output
        .files
        .iter()
        .find(|(n, _)| n == "LandingPage.tsx")
        .unwrap();
    assert!(
        landing.1.contains("landing-mockup.png"),
        "LandingPage should reference the image"
    );
}
