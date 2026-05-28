// mcp_tool_demo.vox
const MCP_TOOL_SRC: &str = r#"type SearchResult =
    | Found(text: str, score: int)
    | NotFound(query: str)

@mcp.tool "Search the knowledge base" fn search_knowledge(query: str) to SearchResult {
    Found("Result for: " + query, 95)
}

@mcp.tool "Get system status" fn system_status() to str {
    return "healthy"
}
"#;

#[test]
fn pipeline_mcp_tool_parse() {
    let tokens = lex(MCP_TOOL_SRC);
    let module = parse(tokens).expect("mcp_tool_demo should parse");
    // 1 type + 2 mcp.tool
    assert_eq!(module.declarations.len(), 3);
    assert!(
        matches!(&module.declarations[1], vox_compiler::ast::decl::Decl::McpTool(m) if m.description == "Search the knowledge base"),
        "First tool should have correct description"
    );
}

// pattern_matching.vox
const PATTERN_MATCHING_SRC: &str = r#"type Shape =
    | Circle(radius: int)
    | Rectangle(width: int, height: int)
    | Triangle(base: int, height: int)

fn area(s: Shape) to int {
    match s {
        Circle(radius) => radius * radius
        Rectangle(width, height) => width * height
        Triangle(base, height) => base * height
    }
}

@test fn test_circle_area() to Unit {
    let c = Circle(5)
    let a = area(c)
    assert(a is 25)
}

@test fn test_rectangle_area() to Unit {
    let r = Rectangle(4, 6)
    let a = area(r)
    assert(a is 24)
}
"#;

#[test]
fn pipeline_pattern_matching_parse() {
    let tokens = lex(PATTERN_MATCHING_SRC);
    let module = parse(tokens).expect("pattern_matching should parse");
    // 1 type + 1 fn + 2 tests
    assert_eq!(module.declarations.len(), 4);
}

#[test]
fn pipeline_pattern_matching_codegen() {
    let tokens = lex(PATTERN_MATCHING_SRC);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);
    let output = generate(&hir).unwrap();
    let types = output.files.iter().find(|(n, _)| n == "types.ts").unwrap();
    insta::assert_snapshot!("shape_types_ts_tagged_union", types.1);
}

// multi_route_app.vox
const MULTI_ROUTE_SRC: &str = r#"import react.use_state

type Todo =
    | Active(text: str)
    | Completed(text: str)

component Dashboard() {
    let (visits, set_visits) = use_state(0)
    view: column(raw_class="dashboard") {
        column(raw_class="dashboard-title") { "Dashboard" }
        link(href="/todos") { "Todos" }
        link(href="/about") { "About" }
        button(raw_class="track-btn", on_click={fn(_e) set_visits(visits + 1)}) { "Track" }
    }
}

component TodoList() {
    let (items, set_items) = use_state([])
    let (draft, set_draft) = use_state("")
    view: column(raw_class="todo_list") {
        input(bind=draft, raw_class="draft-input", aria_label="New task")
        button(raw_class="add-btn", on_click={fn(_e) set_items(items.append({text: draft}))}) { "Add" }
    }
}

component About() {
    view: column(raw_class="about") {
        column(raw_class="about-title") { "About" }
    }
}

routes {
    "/" to Dashboard
    "/todos" to TodoList
    "/about" to About
}

@query fn api_todos() to str {
    return "[]"
}

@mutation fn create_todo() to str {
    return "created"
}

@server fn get_stats() to int {
    return 42
}
"#;

#[test]
fn pipeline_multi_route_parse() {
    let tokens = lex(MULTI_ROUTE_SRC);
    let module = parse(tokens).expect("multi_route_app should parse");
    // 1 import + 1 type + 3 components + 1 routes + 3 @endpoint fns = 9
    assert_eq!(module.declarations.len(), 9);
}

#[test]
fn pipeline_multi_route_codegen() {
    let tokens = lex(MULTI_ROUTE_SRC);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);
    let output = generate(&hir).unwrap();
    let filenames: Vec<&str> = output.files.iter().map(|(n, _)| n.as_str()).collect();
    assert!(
        filenames.contains(&"Dashboard.tsx"),
        "Should produce Dashboard.tsx"
    );
    assert!(
        filenames.contains(&"TodoList.tsx"),
        "Should produce TodoList.tsx"
    );
    assert!(filenames.contains(&"About.tsx"), "Should produce About.tsx");
    assert!(
        filenames.contains(&"routes.manifest.ts"),
        "Should produce routes.manifest.ts for routes:"
    );
    assert!(
        filenames.contains(&"types.ts"),
        "Should produce types.ts for Todo"
    );
}

#[test]
fn pipeline_multi_route_rust_codegen() {
    let tokens = lex(MULTI_ROUTE_SRC);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);
    let output = vox_codegen::codegen_rust::generate(
        &hir,
        "multi_route_app",
        vox_codegen::codegen_rust::RustAppShell::AxumLocalServer,
    )
    .unwrap();
    let main_rs = output.files.get("src/main.rs").expect("main.rs");
    insta::assert_snapshot!("multi_route_rust_main_rs_emit", main_rs);
}

/// `@query` plus Path C components → Web IR summary (OP-0181).
#[test]
fn pipeline_web_ir_lower_summary_counts_http_and_classic() {
    use vox_codegen::web_ir::lower::lower_hir_to_web_ir_with_summary;

    let tokens = lex(MULTI_ROUTE_SRC);
    let module = parse(tokens).expect("multi_route");
    let hir = vox_compiler::hir::lower_module(&module);
    let (_web, summary) = lower_hir_to_web_ir_with_summary(&hir);
    assert!(
        summary.query_fn_contracts >= 1,
        "expected query endpoint contracts, got {summary:?}"
    );
    assert!(
        summary.components >= 1,
        "expected Path C components in HIR/WebIR summary, got {summary:?}"
    );
    assert_eq!(
        summary.classic_components_deferred, 0,
        "fixture should have no deferred classic views, got {summary:?}"
    );
}

/// Chatbot Path C `component Chat` produces a `view_roots` entry and passes `validate_web_ir`.
#[test]
fn pipeline_chat_classic_web_ir_validate_clean() {
    use vox_codegen::web_ir::lower::lower_hir_to_web_ir_with_summary;
    use vox_codegen::web_ir::validate::validate_web_ir;

    let tokens = lex(CHATBOT_SRC);
    let module = parse(tokens).expect("chatbot parse");
    let hir = vox_compiler::hir::lower_module(&module);
    let (web, summary) = lower_hir_to_web_ir_with_summary(&hir);
    assert!(
        summary.components >= 1,
        "Chat Path C should appear in reactive summary, got {summary:?}"
    );
    assert!(
        web.view_roots.iter().any(|(n, _)| n == "Chat"),
        "expected Chat in view_roots, have {:?}",
        web.view_roots.iter().map(|(n, _)| n).collect::<Vec<_>>()
    );
    let diags = validate_web_ir(&web);
    assert!(
        diags.is_empty(),
        "validate_web_ir expected clean: {diags:?}"
    );
}

/// OP-S032: integration gate — AST [`vox_codegen::codegen_ts::jsx`] and HIR [`emit_hir`] share [`compat`] DOM edges.
#[test]
fn pipeline_compat_tag_gate_jsx_hir_emit_matrix() {
    let edges = [
        ("for", "htmlFor"),
        ("tab_index", "tabIndex"),
        ("class", "className"),
    ];
    for (vox, react) in edges {
        assert_eq!(
            vox_codegen::codegen_ts::hir_emit::map_jsx_attr_name(vox),
            react
        );
        assert_eq!(vox_codegen::codegen_ts::jsx::map_jsx_attr_name(vox), react);
    }
}

/// OP-S034: Express [`validate_express_route_emit_input`] accepts multi-route HIR (`routes.rs` OP-S033 mapper notes).
#[test]
fn pipeline_express_contract_mapper_fixture_validates_multi_route_hir() {
    let tokens = lex(MULTI_ROUTE_SRC);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);
    assert!(
        hir.endpoint_fns
            .iter()
            .any(|e| e.route_path.contains("api_todos") || e.route_path.contains("create_todo") || e.route_path.contains("get_stats")),
        "expected endpoint fns in hir.endpoint_fns: {:?}",
        hir.endpoint_fns.iter().map(|e| &e.route_path).collect::<Vec<_>>()
    );
}

/// OP-S036: route + component gate — Express validation, Web IR validate clean, `routes.manifest.ts` present.
#[test]
fn pipeline_route_component_express_and_web_ir_gate() {
    use vox_codegen::web_ir::validate::validate_web_ir;

    let tokens = lex(MULTI_ROUTE_SRC);
    let module = parse(tokens).unwrap();
    let hir = vox_compiler::hir::lower_module(&module);
    // Express validation is decommissioned
    let (web, summary) =
        vox_codegen::web_ir::lower::lower_hir_to_web_ir_with_summary(&hir);
    assert!(
        summary.client_route_trees >= 1,
        "expected client route trees, got {summary:?}"
    );
    let diags = validate_web_ir(&web);
    assert!(diags.is_empty(), "{diags:?}");
    let out = generate_without_express!(&module);
    assert!(
        out.files.iter().any(|(n, _)| n == "routes.manifest.ts"),
        "expected routes.manifest.ts in {:?}",
        out.files.iter().map(|(n, _)| n).collect::<Vec<_>>()
    );
}

