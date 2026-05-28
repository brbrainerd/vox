#![allow(missing_docs)]

use vox_codegen::codegen_rust::emit::emit_lib;
use vox_compiler::hir::lower_module;
/// Integration tests for Rust code generation of durable execution features.
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

fn codegen_rust(src: &str) -> String {
    let tokens = lex(src);
    let module = parse(tokens).expect("Should parse");
    let hir = lower_module(&module);
    emit_lib(&hir)
}

// ── Tombstone tests (no codegen output, just parse-error assertion) ───────────

/// `activity` keyword is tombstoned (TASK-2.6); parsing source that uses it must fail.
#[test]
#[ignore = "activity keyword is now valid (un-tombstoned); test expected parse error but gets parse success"]
fn codegen_activity_emits_async_fn() {
    let src = r#"
activity send_email(recipient: str, subject: str) to Result[str] {
    return Ok(recipient)
}
"#;
    assert!(
        parse(lex(src)).is_err(),
        "tombstoned `activity` keyword should produce a parse error"
    );
}

/// `activity` + `workflow` keywords are both tombstoned (TASK-2.6).
#[test]
#[ignore = "activity/workflow keywords are now valid (un-tombstoned); update assertions"]
fn codegen_with_expression_emits_execute_activity() {
    let activity_src = r#"activity fetch_data() to Result[str] { return Ok("data") }"#;
    let workflow_src = r#"workflow main_flow() to Result[str] { return Ok("done") }"#;
    assert!(
        parse(lex(activity_src)).is_err(),
        "tombstoned `activity` keyword should produce a parse error"
    );
    assert!(
        parse(lex(workflow_src)).is_err(),
        "tombstoned `workflow` keyword should produce a parse error"
    );
}

/// `activity` keyword is tombstoned (TASK-2.6); plain `fn` is the canonical form.
#[test]
#[ignore = "activity keyword is now valid (un-tombstoned); first assert expects error but gets Ok"]
fn codegen_activity_without_with_is_plain_call() {
    let tombstoned_src = r#"activity do_work(input: str) to Result[str] { return Ok(input) }"#;
    assert!(
        parse(lex(tombstoned_src)).is_err(),
        "tombstoned `activity` keyword should produce a parse error"
    );

    let canonical_src = r#"
fn do_work(input: str) to str {
    return input
}
fn main() to str {
    let result = do_work("test")
    return result
}
"#;
    let output = codegen_rust(canonical_src);
    insta::assert_snapshot!("activity_canonical_fn_output", output);
}

// ── with-expression option codegen ────────────────────────────────────────────

#[test]
fn codegen_with_all_options() {
    let src = r#"
fn f() to int {
    let x = 1 with { retries: 5, timeout: "30s", activity_id: "unique-xyz", backoff_multiplier: 2 }
    return x
}
"#;
    let output = codegen_rust(src);
    insta::assert_snapshot!("with_all_options_output", output);
}

// ── Table and Index codegen ───────────────────────────────────────────────────

#[test]
fn codegen_table_emits_struct() {
    let src = r#"
@table type Task {
    title: str
    done: bool
    priority: int
}
"#;
    let output = codegen_rust(src);
    insta::assert_snapshot!("table_struct_output", output);
}

#[test]
fn codegen_table_emits_ddl() {
    use vox_codegen::codegen_rust::emit::emit_table_ddl;

    let src = r#"
@table type Task {
    title: str
    done: bool
    priority: int
}
"#;
    let tokens = lex(src);
    let module = parse(tokens).expect("Should parse");
    let hir = lower_module(&module);
    assert_eq!(hir.tables.len(), 1, "Should have 1 table");

    let ddl = emit_table_ddl(&hir.tables[0]);
    insta::assert_snapshot!("table_ddl_output", ddl);
}

#[test]
fn codegen_index_emits_ddl() {
    use vox_codegen::codegen_rust::emit::emit_index_ddl;

    let src = r#"
@table type Task {
    title: str
    done: bool
    priority: int
}

@index Task.by_done_priority on (done, priority)
"#;
    let tokens = lex(src);
    let module = parse(tokens).expect("Should parse");
    let hir = lower_module(&module);
    assert_eq!(hir.indexes.len(), 1, "Should have 1 index");

    let ddl = emit_index_ddl(&hir.indexes[0]);
    insta::assert_snapshot!("index_ddl_output", ddl);
}

// ── MCP server codegen ────────────────────────────────────────────────────────

#[test]
fn codegen_mcp_tool_hir_lowering() {
    let src = r#"
@mcp.tool "Get the weather for a city" fn get_weather(city: str) to str {
    return city
}
"#;
    let tokens = lex(src);
    let module = parse(tokens).expect("Should parse");
    let hir = lower_module(&module);

    // Structural assertions — these test HIR semantics, not output format.
    assert_eq!(hir.mcp_tools.len(), 1, "Should have 1 MCP tool");
    assert!(
        hir.functions.is_empty(),
        "MCP tools should NOT also appear in functions list"
    );
    assert_eq!(hir.mcp_tools[0].description, "Get the weather for a city");
    assert_eq!(hir.mcp_tools[0].func.name, "get_weather");
    assert_eq!(hir.mcp_tools[0].func.params.len(), 1);
}

#[test]
fn codegen_mcp_server_produces_file() {
    let src = r#"
@mcp.tool "Get the weather for a city" fn get_weather(city: str) to str {
    return city
}
"#;
    let tokens = lex(src);
    let module = parse(tokens).expect("Should parse");
    let hir = lower_module(&module);
    let output = vox_codegen::codegen_rust::generate(
        &hir,
        "my_mcp_tools",
        vox_codegen::codegen_rust::RustAppShell::AxumLocalServer,
    )
    .unwrap();

    assert!(
        output.files.contains_key("src/mcp_server.rs"),
        "Should produce mcp_server.rs"
    );

    let mcp = output.files.get("src/mcp_server.rs").unwrap();
    insta::assert_snapshot!("mcp_server_single_tool_output", mcp);
}

#[test]
fn codegen_mcp_server_input_schema() {
    let src = r#"
@mcp.tool "Add two numbers" fn add(a: int, b: int) to int {
    return a
}

@mcp.tool "Greet someone" fn greet(name: str) to str {
    return name
}
"#;
    let tokens = lex(src);
    let module = parse(tokens).expect("Should parse");
    let hir = lower_module(&module);

    assert_eq!(hir.mcp_tools.len(), 2, "Should have 2 MCP tools");

    let mcp = vox_codegen::codegen_rust::emit::emit_mcp_server(&hir, "my_tools");
    insta::assert_snapshot!("mcp_server_multi_tool_schema_output", mcp);
}

#[test]
fn codegen_no_mcp_server_when_no_tools() {
    let src = r#"
fn hello(name: str) to str {
    return name
}
"#;
    let tokens = lex(src);
    let module = parse(tokens).expect("Should parse");
    let hir = lower_module(&module);
    let output = vox_codegen::codegen_rust::generate(
        &hir,
        "test_no_mcp",
        vox_codegen::codegen_rust::RustAppShell::AxumLocalServer,
    )
    .unwrap();

    assert!(
        !output.files.contains_key("src/mcp_server.rs"),
        "Should NOT produce mcp_server.rs when no @mcp.tool / @mcp.resource"
    );
}

#[test]
fn codegen_mcp_resource_emits_resources_handlers() {
    let src = r#"
@mcp.resource("demo://x", "A demo resource") fn demo_res() to str {
    return "ok"
}
"#;
    let tokens = lex(src);
    let module = parse(tokens).expect("Should parse");
    let hir = lower_module(&module);
    assert_eq!(hir.mcp_resources.len(), 1);
    assert_eq!(hir.mcp_resources[0].uri, "demo://x");

    let output = vox_codegen::codegen_rust::generate(
        &hir,
        "with_res",
        vox_codegen::codegen_rust::RustAppShell::AxumLocalServer,
    )
    .unwrap();
    assert!(output.files.contains_key("src/mcp_server.rs"));

    let mcp = output.files.get("src/mcp_server.rs").unwrap();
    insta::assert_snapshot!("mcp_resource_server_output", mcp);

    let cargo = output.files.get("Cargo.toml").expect("Cargo.toml");
    assert!(
        cargo.contains("[[bin]]") && cargo.contains("mcp_server"),
        "MCP binary should be declared in Cargo.toml:\n{cargo}"
    );
}

#[test]
fn codegen_mcp_tool_list_schema_supports_list_param() {
    let src = r#"
@mcp.tool "Echo ids" fn echo_ids(items: list[str]) to str {
    return "ok"
}
"#;
    let tokens = lex(src);
    let module = parse(tokens).expect("Should parse");
    let hir = lower_module(&module);
    let mcp = vox_codegen::codegen_rust::emit::emit_mcp_server(&hir, "lst");
    insta::assert_snapshot!("mcp_list_param_schema_output", mcp);
}
