#[test]
fn pipeline_lex_produces_tokens() {
    let tokens = lex(CHATBOT_SRC);
    assert!(
        tokens.len() > 100,
        "Expected many tokens, got {}",
        tokens.len()
    );
}

#[test]
fn pipeline_parse_produces_five_declarations() {
    let tokens = lex(CHATBOT_SRC);
    let module = parse(tokens).expect("Should parse without errors");
    // import, type, component, @endpoint server, @endpoint mutation
    assert_eq!(
        module.declarations.len(),
        5,
        "import + type + component + server endpoint + mutation endpoint"
    );
}

#[test]
fn pipeline_typecheck_has_no_errors() {
    let tokens = lex(CHATBOT_SRC);
    let module = parse(tokens).unwrap();
    let diagnostics = typecheck_module(&module, "");
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == vox_compiler::typeck::diagnostics::TypeckSeverity::Error)
        .collect();
    assert!(
        errors.is_empty(),
        "Should have no type errors: {:?}",
        errors
    );
}

#[test]
fn pipeline_codegen_produces_chatbot_ts_bundle_without_express() {
    let tokens = lex(CHATBOT_SRC);
    let module = parse(tokens).unwrap();
    let output = generate_without_express!(&module);
    let filenames: Vec<&str> = output.files.iter().map(|(n, _)| n.as_str()).collect();
    assert!(filenames.contains(&"types.ts"), "Should produce types.ts");
    assert!(
        filenames.contains(&"vox-app-contract.json"),
        "Should emit app contract JSON"
    );
    assert!(
        filenames.contains(&"vox-tanstack-query.tsx"),
        "Should emit TanStack query helper"
    );
    assert!(filenames.contains(&"Chat.tsx"), "Should produce Chat.tsx");
    assert!(
        !filenames.contains(&"server.ts"),
        "server.ts should not be emitted unless VOX_EMIT_EXPRESS_SERVER=1"
    );
}

#[test]
fn codegen_types_has_tagged_union() {
    let tokens = lex(CHATBOT_SRC);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);
    let output = generate(&hir).unwrap();

    let types = output.files.iter().find(|(n, _)| n == "types.ts").unwrap();
    insta::assert_snapshot!("chatbot_types_ts_tagged_union", types.1);
}

#[test]
fn codegen_component_has_use_state() {
    let tokens = lex(CHATBOT_SRC);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);
    let output = generate(&hir).unwrap();

    let chat = output.files.iter().find(|(n, _)| n == "Chat.tsx").unwrap();
    insta::assert_snapshot!("chatbot_chat_tsx_usestate", chat.1);
}

#[test]
fn codegen_server_has_express_route_with_await() {
    // Express server emission (`server.ts`) was fully decommissioned — `VOX_EMIT_EXPRESS_SERVER`
    // is no longer read by vox-codegen. HTTP routing is handled by Axum, not Express.
    // This test now verifies that `server.ts` is NOT emitted even when the env-var is set,
    // and that the generate call itself still succeeds (no panic / error).
    let tokens = lex(CHATBOT_SRC);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);
    let output = with_express_server_enabled(|| generate(&hir).unwrap());
    assert!(
        !output.files.iter().any(|(n, _)| n == "server.ts"),
        "Express server.ts must not be emitted (express decommissioned); got: {:?}",
        output.files.iter().map(|(n, _)| n).collect::<Vec<_>>()
    );
}

#[test]
fn codegen_jsx_text_content_not_interpolated() {
    let tokens = lex(CHATBOT_SRC);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);
    let output = generate(&hir).unwrap();

    let chat = output.files.iter().find(|(n, _)| n == "Chat.tsx").unwrap();
    // Text content like "Vox" and "Chatbot" inside <h1> should appear as plain text,
    // NOT as {Vox} or {Chatbot} JSX expressions
    assert!(
        !chat.1.contains("{Vox}"),
        "Bare text should not be wrapped in braces"
    );
    assert!(
        !chat.1.contains("{Chatbot}"),
        "Bare text should not be wrapped in braces"
    );
    assert!(
        !chat.1.contains("{Send}"),
        "Button text should not be wrapped in braces"
    );
}

// --- TS codegen for activities (tombstoned: activity construct removed) ---

#[test]
#[ignore = "activity construct tombstoned; server-side logic uses @endpoint(kind: mutation) fn"]
fn codegen_ts_activity_produces_activities_file() {
    let src = r#"
type MyRes = | Ok(v: str) | Error

activity send_email(recipient: str, subject: str) to Result[str] {
    return Ok(recipient)
}
"#;
    let tokens = lex(src);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);
    let output = generate(&hir).unwrap();

    let filenames: Vec<&str> = output.files.iter().map(|(n, _)| n.as_str()).collect();
    assert!(
        filenames.contains(&"activities.ts"),
        "Should produce activities.ts, got: {:?}",
        filenames
    );
}

#[test]
#[ignore = "activity construct tombstoned; server-side logic uses @endpoint(kind: mutation) fn"]
fn codegen_ts_activity_has_async_function() {
    let src = r#"
type MyRes = | Ok(v: str) | Error

activity fetch_data(url: str) to Result[str] {
    return Ok(url)
}
"#;
    let tokens = lex(src);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);
    let output = generate(&hir).unwrap();

    let activities = output
        .files
        .iter()
        .find(|(n, _)| n == "activities.ts")
        .unwrap();
    insta::assert_snapshot!("activity_fetch_data_ts_emit", activities.1);
}

#[test]
#[ignore = "activity construct tombstoned; server-side logic uses @endpoint(kind: mutation) fn"]
fn codegen_ts_activity_has_runtime_helper() {
    let src = r#"
type MyRes = | Ok(v: str) | Error

activity do_work() to Result[str] {
    return Ok("done")
}
"#;
    let tokens = lex(src);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);
    let output = generate(&hir).unwrap();

    let activities = output
        .files
        .iter()
        .find(|(n, _)| n == "activities.ts")
        .unwrap();
    insta::assert_snapshot!("activity_do_work_ts_runtime_helpers", activities.1);
}

// --- TS codegen for tables ---

#[test]
fn codegen_ts_table_produces_schema_file() {
    let src = r#"
@table type Task {
    title: str
    done: bool
    priority: int
}
"#;
    let tokens = lex(src);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);
    let output = generate(&hir).unwrap();

    let filenames: Vec<&str> = output.files.iter().map(|(n, _)| n.as_str()).collect();
    assert!(
        filenames.contains(&"schema.ts"),
        "Should produce schema.ts, got: {:?}",
        filenames
    );

    let schema = output.files.iter().find(|(n, _)| n == "schema.ts").unwrap();
    insta::assert_snapshot!("table_task_schema_ts_emit", schema.1);
}

// --- @v0 codegen tests ---

#[test]
#[ignore = "@v0 components dropped from HIR (Path B removed); no TSX generated — owner: integration-tests sunset: 2026-12-31"]
fn codegen_v0_placeholder_from_prompt() {
    let src = r#"@v0 "A stats dashboard with charts" Stats {}"#;
    let tokens = lex(src);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);
    let output = generate(&hir).unwrap();

    let filenames: Vec<&str> = output.files.iter().map(|(n, _)| n.as_str()).collect();
    assert!(
        filenames.contains(&"Stats.tsx"),
        "Should produce Stats.tsx, got: {:?}",
        filenames
    );

    let stats = output.files.iter().find(|(n, _)| n == "Stats.tsx").unwrap();
    insta::assert_snapshot!("v0_stats_tsx_placeholder", stats.1);
}

#[test]
#[ignore = "@v0 components dropped from HIR (Path B removed); no TSX generated — owner: integration-tests sunset: 2026-12-31"]
fn codegen_v0_placeholder_from_image() {
    let src = r#"@v0 from "design.png" Dashboard {}"#;
    let tokens = lex(src);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);
    let output = generate(&hir).unwrap();

    let dash = output
        .files
        .iter()
        .find(|(n, _)| n == "Dashboard.tsx")
        .unwrap();
    insta::assert_snapshot!("v0_dashboard_tsx_from_image", dash.1);
}

// --- @table / @index end-to-end pipeline tests ---

const DATA_LAYER_SRC: &str = r#"@table type Task {
    title: str
    done: bool
    priority: int
}

@index Task.by_done on (done, priority)
"#;

#[test]
fn pipeline_table_parse_produces_declarations() {
    let tokens = lex(DATA_LAYER_SRC);
    let module = parse(tokens).expect("Should parse @table source");
    assert_eq!(module.declarations.len(), 2, "table + index");
}

#[test]
fn pipeline_table_typecheck_no_errors() {
    let tokens = lex(DATA_LAYER_SRC);
    let module = parse(tokens).unwrap();
    let diagnostics = typecheck_module(&module, "");
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == vox_compiler::typeck::diagnostics::TypeckSeverity::Error)
        .collect();
    assert!(
        errors.is_empty(),
        "Should have no type errors for @table: {:?}",
        errors
    );
}

#[test]
fn pipeline_table_hir_lowering() {
    let tokens = lex(DATA_LAYER_SRC);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);

    assert_eq!(hir.tables.len(), 1, "one table");
    assert_eq!(hir.tables[0].name, "Task");
    assert_eq!(hir.tables[0].fields.len(), 3);
    assert_eq!(hir.indexes.len(), 1, "one index");
    assert_eq!(hir.indexes[0].table_name, "Task");
    assert_eq!(hir.indexes[0].index_name, "by_done");
}

#[test]
fn pipeline_table_rust_codegen_e2e() {
    let tokens = lex(DATA_LAYER_SRC);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);
    let output = vox_codegen::codegen_rust::generate(
        &hir,
        "test_data",
        vox_codegen::codegen_rust::RustAppShell::AxumLocalServer,
    )
    .unwrap();

    let lib_rs = output.files.get("src/lib.rs").expect("lib.rs");
    insta::assert_snapshot!("table_task_lib_rs_emit", lib_rs);

    let main_rs = output.files.get("src/main.rs").expect("main.rs");
    insta::assert_snapshot!("table_task_main_rs_emit", main_rs);
}

// --- routes codegen test ---

#[test]
fn codegen_routes_produces_route_manifest_ts() {
    // Component stubs satisfy validate_manifest_symbols (wired in 7ba386cfc5).
    let src = "fn home() to str { return \"\" }\nfn about() to str { return \"\" }\nroutes {\n    \"/\" to home\n    \"/about\" to about\n}";
    let tokens = lex(src);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);
    let output = with_web_ir_validate_cleared(|| generate(&hir).unwrap());

    let filenames: Vec<&str> = output.files.iter().map(|(n, _)| n.as_str()).collect();
    assert!(
        filenames.contains(&"routes.manifest.ts"),
        "Should produce routes.manifest.ts, got: {:?}",
        filenames
    );

    let m = output
        .files
        .iter()
        .find(|(n, _)| n == "routes.manifest.ts")
        .unwrap();
    insta::assert_snapshot!("routes_home_about_manifest_ts", m.1);
}

#[test]
fn codegen_routes_with_loading_emits_pending_component() {
    // home stub satisfies validate_manifest_symbols (wired in 7ba386cfc5).
    let src = r#"@loading fn Spinner() to Element { return column(raw_class="spinner") { "wait" } }

fn home() to str { return "" }

routes {
    "/" to home
}
"#;
    let tokens = lex(src);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);
    let output = with_web_ir_validate_cleared(|| generate(&hir).unwrap());
    let m = output
        .files
        .iter()
        .find(|(n, _)| n == "routes.manifest.ts")
        .unwrap();
    insta::assert_snapshot!("routes_with_loading_spinner_manifest_ts", m.1);
    // @loading fn is a Path B surface, dropped from HIR lowering: no Spinner.tsx is emitted.
    assert!(
        !output.files.iter().any(|(n, _)| n == "Spinner.tsx"),
        "@loading is Path B (dropped from HIR); Spinner.tsx must not appear in output"
    );
}

#[test]
fn codegen_tanstack_start_flag_does_not_emit_separate_router_file() {
    // Component stubs satisfy validate_manifest_symbols (wired in 7ba386cfc5).
    let src = "fn home() to str { return \"\" }\nfn about() to str { return \"\" }\nroutes {\n    \"/\" to home\n    \"/about\" to about\n}";
    let tokens = lex(src);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);
    let output = with_web_ir_validate_cleared(|| {
        generate_with_options(
            &hir,
            CodegenOptions {
                tanstack_start: true,
                ..Default::default()
            },
        )
        .unwrap()
    });

    let filenames: Vec<&str> = output.files.iter().map(|(n, _)| n.as_str()).collect();
    assert!(
        filenames.contains(&"routes.manifest.ts"),
        "Should produce routes.manifest.ts, got: {:?}",
        filenames
    );
    assert!(
        !filenames.contains(&"VoxTanStackRouter.tsx"),
        "Legacy VoxTanStackRouter.tsx must not be emitted, got: {:?}",
        filenames
    );
    assert!(
        !filenames.contains(&"App.tsx"),
        "Compiler must not emit App.tsx; user-owned adapter only, got: {:?}",
        filenames
    );
}

#[test]
fn golden_web_routing_fullstack_codegen_emits_manifest_and_client() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/golden/web_routing_fullstack.vox");
    let src = read_utf8_path_capped(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let tokens = lex(&src);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);
    let output = generate(&hir).unwrap();
    let names: Vec<&str> = output.files.iter().map(|(n, _)| n.as_str()).collect();
    assert!(
        names.contains(&"routes.manifest.ts"),
        "expected routes.manifest.ts, got {names:?}"
    );
    assert!(
        names.contains(&"vox-client.ts"),
        "expected vox-client.ts for @query, got {names:?}"
    );
    let client = output
        .files
        .iter()
        .find(|(n, _)| n == "vox-client.ts")
        .map(|(_, c)| c.as_str())
        .expect("vox-client.ts");
    insta::assert_snapshot!("web_routing_fullstack_client_ts", client);
    let manifest = output
        .files
        .iter()
        .find(|(n, _)| n == "routes.manifest.ts")
        .map(|(_, c)| c.as_str())
        .expect("routes.manifest.ts");
    insta::assert_snapshot!("web_routing_fullstack_manifest_ts", manifest);
}

#[test]
fn golden_blog_fullstack_codegen_emits_manifest_get_and_post() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/golden/blog_fullstack.vox");
    let src = read_utf8_path_capped(&path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let tokens = lex(&src);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);
    let output = generate(&hir).unwrap();
    let names: Vec<&str> = output.files.iter().map(|(n, _)| n.as_str()).collect();
    assert!(
        names.contains(&"routes.manifest.ts"),
        "expected routes.manifest.ts, got {names:?}"
    );
    assert!(
        names.contains(&"vox-client.ts"),
        "expected vox-client.ts, got {names:?}"
    );
    let client = output
        .files
        .iter()
        .find(|(n, _)| n == "vox-client.ts")
        .map(|(_, c)| c.as_str())
        .expect("vox-client.ts");
    insta::assert_snapshot!("blog_fullstack_client_ts", client);
}

// --- bind={} reactive binding test ---

#[test]
fn codegen_bind_expands_to_value_onchange() {
    let src = r#"component LoginForm() {
    let (email, set_email) = use_state("")
    view: input(bind=email, raw_class="email-field")
}"#;
    let tokens = lex(src);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);
    let output = with_web_ir_validate_cleared(|| generate(&hir).unwrap());

    let component = output
        .files
        .iter()
        .find(|(n, _)| n == "LoginForm.tsx")
        .unwrap();
    insta::assert_snapshot!("bind_loginform_tsx_expand", component.1);
}

// --- use_effect hook mapping test ---

#[test]
fn codegen_use_effect_maps_to_react_hook() {
    let src = r#"component Timer() {
    let (count, set_count) = use_state(0)
    use_effect(fn(_x) count)
    view: column(raw_class="timer") { count }
}"#;
    let tokens = lex(src);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);
    let output = generate(&hir).unwrap();

    let component = output.files.iter().find(|(n, _)| n == "Timer.tsx").unwrap();
    insta::assert_snapshot!("use_effect_timer_tsx_emit", component.1);
}

// --- Dashboard full-pipeline integration test ---

#[test]
fn dashboard_full_pipeline_e2e() {
    let src = "type Message = | User(text: str) | Bot(text: str)\n\ncomponent Dashboard() {\n    state n: int = 0\n    view: column(raw_class=\"dash\") { n }\n}\n\ncomponent ChatWidget() {\n    let (messages, set_messages) = use_state([])\n    let (input, set_input) = use_state(\"\")\n    view: column(raw_class=\"chat\") {\n        input(bind=input, raw_class=\"chat-input\", aria_label=\"Chat message\")\n        button(raw_class=\"send-btn\", on_click={fn(e) set_input(\"\")}) { \"Send\" }\n    }\n}\n\n@endpoint(kind: query) fn api_stats() to str {\n    return \"[]\"\n}\n\nroutes {\n    \"/\" to Dashboard\n    \"/chat\" to ChatWidget\n}";

    let tokens = lex(src);
    let module = parse(tokens).unwrap();
    let output = generate_without_express!(&module);

    let filenames: Vec<&str> = output.files.iter().map(|(n, _)| n.as_str()).collect();

    // All expected output files
    assert!(
        filenames.contains(&"types.ts"),
        "Should produce types.ts, got: {:?}",
        filenames
    );
    assert!(
        filenames.contains(&"Dashboard.tsx"),
        "Should produce Dashboard.tsx for component"
    );
    assert!(
        filenames.contains(&"ChatWidget.tsx"),
        "Should produce ChatWidget.tsx for component"
    );
    assert!(
        !filenames.contains(&"server.ts"),
        "Express server.ts is opt-in (VOX_EMIT_EXPRESS_SERVER); http routes are served by Axum"
    );
    assert!(
        filenames.contains(&"routes.manifest.ts"),
        "Should produce routes.manifest.ts for routes:"
    );

    let dash = output
        .files
        .iter()
        .find(|(n, _)| n == "Dashboard.tsx")
        .unwrap();
    insta::assert_snapshot!("dashboard_e2e_dash_tsx", dash.1);

    // @component with bind={}
    let chat = output
        .files
        .iter()
        .find(|(n, _)| n == "ChatWidget.tsx")
        .unwrap();
    insta::assert_snapshot!("dashboard_e2e_chatwidget_tsx", chat.1);

    // routes -> routes.manifest.ts
    let m = output
        .files
        .iter()
        .find(|(n, _)| n == "routes.manifest.ts")
        .unwrap();
    insta::assert_snapshot!("dashboard_e2e_routes_manifest_ts", m.1);

    // types.ts
    let types = output.files.iter().find(|(n, _)| n == "types.ts").unwrap();
    insta::assert_snapshot!("dashboard_e2e_types_ts", types.1);
}

#[test]
fn chatbot_full_pipeline_e2e() {
    // This test builds the actual examples/chatbot.vox file
    // We assume the test runner is executed from workspace root or crate root
    // But TestProject usually handles tmp dir.
    // We need to read the file content manually if we use lex/parse directly?
    // Or just use the file path if we had a helper.
    // Since previous tests verify logic using inline strings, we'll read the file content here.

    let path = Path::new("fixtures/chatbot.vox");
    let src = read_utf8_path_capped(path)
        .or_else(|_| read_utf8_path_capped(Path::new("tests/fixtures/chatbot.vox")))
        .expect("Could not read chatbot.vox fixture");

    let tokens = vox_compiler::lexer::cursor::lex(&src);
    let module = vox_compiler::parser::parse(tokens).expect("Chatbot should parse");

    let diagnostics = vox_compiler::typeck::typecheck_module(&module, "");
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == vox_compiler::typeck::diagnostics::TypeckSeverity::Error)
        .collect();
    assert!(
        errors.is_empty(),
        "Chatbot should have no type errors: {:?}",
        errors
    );

    let output = generate_without_express!(&module);

    let filenames: Vec<&str> = output.files.iter().map(|(n, _)| n.as_str()).collect();

    assert!(filenames.contains(&"Chat.tsx"), "Should produce Chat.tsx");
    assert!(
        filenames.contains(&"Chat.css"),
        "Should produce Chat.css (from style block)"
    );
    assert!(
        filenames.contains(&"routes.manifest.ts"),
        "Should produce routes.manifest.ts (from routes)"
    );

    let chat_css = output.files.iter().find(|(n, _)| n == "Chat.css").unwrap();
    insta::assert_snapshot!("chatbot_fixture_chat_css", chat_css.1);

    let chat_tsx = output.files.iter().find(|(n, _)| n == "Chat.tsx").unwrap();
    insta::assert_snapshot!("chatbot_fixture_chat_tsx", chat_tsx.1);
}

/// Path C `component` surfaces + client `routes` + HTTP route
/// (blueprint OP-0037 / OP-0047 / OP-0289 family).
const MIXED_SURFACE_SRC: &str = r#"
import react.use_state

component Dash() {
    state n: int = 0
    view: column(raw_class="dashboard") {
        n
    }
}

component Shell() {
    let (x, _set_x) = use_state(0)
    view: column(raw_class="shell") { x }
}

routes {
    "/" to Dash
}

@endpoint(kind: mutation) fn api_x() to str {
    return "ok"
}
"#;

#[test]
fn pipeline_mixed_declarations_lower_without_panic() {
    let tokens = lex(MIXED_SURFACE_SRC);
    let module = parse(tokens).expect("MIXED_SURFACE should parse");
    let diagnostics = typecheck_module(&module, "");
    let errors: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.severity == vox_compiler::typeck::diagnostics::TypeckSeverity::Error)
        .collect();
    assert!(errors.is_empty(), "typecheck: {errors:?}");
    let _hir = vox_compiler::hir::lower_module(&module);
}

#[test]
fn pipeline_mixed_declarations_hir_counts_and_web_ir_validate() {
    use vox_codegen::web_ir::lower::lower_hir_to_web_ir;
    use vox_codegen::web_ir::validate::validate_web_ir;

    let tokens = lex(MIXED_SURFACE_SRC);
    let module = parse(tokens).expect("parse");
    let hir = vox_compiler::hir::lower_module(&module);
    assert!(
        hir.legacy_ast_nodes.is_empty(),
        "unexpected legacy: {:?}",
        hir.legacy_ast_nodes
    );
    assert_eq!(hir.endpoint_fns.len(), 1);


    let web = lower_hir_to_web_ir(&hir);
    let diags = validate_web_ir(&web);
    assert!(diags.is_empty(), "{diags:?}");
}

#[test]
fn pipeline_endpoint_fn_route_path_preserved_for_codegen() {
    let tokens = lex(MIXED_SURFACE_SRC);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);
    assert_eq!(hir.endpoint_fns.len(), 1);
    assert!(
        hir.endpoint_fns[0].route_path.contains("api_x"),
        "route_path should contain function name: {}",
        hir.endpoint_fns[0].route_path
    );
}

#[test]
fn pipeline_mixed_surface_worked_app_web_ir_gate_and_tsx_substrings() {
    with_web_ir_validate_cleared(|| {
        let tokens = lex(MIXED_SURFACE_SRC);
        let module = parse(tokens).unwrap();
        let hir = vox_compiler::hir::lower_module(&module);
        let output = generate(&hir).expect("codegen");
        let dash = output
            .files
            .iter()
            .find(|(n, _)| n == "Dash.tsx")
            .map(|(_, c)| c.as_str())
            .expect("Dash.tsx");
        insta::assert_snapshot!("mixed_surface_dash_tsx", dash);
        let shell = output
            .files
            .iter()
            .find(|(n, _)| n == "Shell.tsx")
            .map(|(_, c)| c.as_str())
            .expect("Shell.tsx");
        insta::assert_snapshot!("mixed_surface_shell_tsx", shell);
        let m = output
            .files
            .iter()
            .find(|(n, _)| n == "routes.manifest.ts")
            .map(|(_, c)| c.as_str())
            .expect("routes.manifest.ts");
        insta::assert_snapshot!("mixed_surface_routes_manifest_ts", m);
    });
}

#[test]
fn pipeline_codegen_without_vox_web_ir_validate_env_succeeds() {
    with_web_ir_validate_cleared(|| {
        let tokens = lex(MIXED_SURFACE_SRC);
        let module = parse(tokens).unwrap();
        let hir = vox_compiler::hir::lower_module(&module);
        generate(&hir).expect("generate with validate env cleared");
    });
}

#[test]
fn pipeline_codegen_with_vox_web_ir_validate_env() {
    with_web_ir_validate_enabled(|| {
        let tokens = lex(MIXED_SURFACE_SRC);
        let module = parse(tokens).unwrap();
        let hir = vox_compiler::hir::lower_module(&module);
        generate(&hir).expect("generate with VOX_WEBIR_VALIDATE=1");
    });
}

#[test]
fn pipeline_mixed_surface_typecheck_without_errors() {
    let tokens = lex(MIXED_SURFACE_SRC);
    let module = parse(tokens).unwrap();
    let diagnostics = typecheck_module(&module, "");
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.severity == vox_compiler::typeck::diagnostics::TypeckSeverity::Error),
        "{diagnostics:?}"
    );
}

fn assert_mixed_surface_codegen_core_files() {
    with_web_ir_validate_cleared(|| {
        let tokens = lex(MIXED_SURFACE_SRC);
        let module = parse(tokens).unwrap();
        let hir = vox_compiler::hir::lower_module(&module);
        let output = generate(&hir).expect("codegen");
        let names: Vec<&str> = output.files.iter().map(|(n, _)| n.as_str()).collect();
        for needle in ["Dash.tsx", "Shell.tsx", "routes.manifest.ts"] {
            assert!(
                names.contains(&needle),
                "expected {needle} in {:?}",
                names
            );
        }
    });
}

#[test]
fn pipeline_mixed_surface_codegen_core_file_manifest() {
    assert_mixed_surface_codegen_core_files();
}

#[test]
fn pipeline_hir_emit_legacy_shrink_public_api_codegen() {
    assert_mixed_surface_codegen_core_files();
}

// --- TanStack Start scaffold (no Node): keep in sync with `vox-cli` `scaffold_tanstack_start_layout` ---

#[test]
fn tanstack_start_scaffold_manifest_writes_file_routes() {
    use std::fs;
    let tmp = tempfile::tempdir().expect("tempdir");
    let ts_out = tmp.path().join("ts_out");
    let app = tmp.path().join("app");
    fs::create_dir_all(&ts_out).expect("ts_out");
    fs::write(
        ts_out.join("routes.manifest.ts"),
        "export const voxRoutes = [] as never[];\n",
    )
    .expect("manifest");
    fs::write(
        ts_out.join("Home.tsx"),
        "export function Home() { return null; }\n",
    )
    .expect("home");
    vox_cli::frontend::scaffold_react_app(&app, &ts_out, true).expect("scaffold");
    assert!(
        app.join("src/routeTree.gen.ts").is_file(),
        "routeTree.gen.ts missing"
    );
    assert!(
        app.join("src/routes/index.tsx").is_file(),
        "Start + manifest uses file routes (index.tsx)"
    );
    assert!(app.join("src/router.tsx").is_file());
}
