use vox_codegen_rust::emit::emit_lib;
use vox_hir::lower_module;
/// Integration tests for Rust code generation of durable execution features.
/// Verifies that activities and `with` expressions produce correct Rust output.
use vox_lexer::cursor::lex;
use vox_parser::parser::parse;

fn codegen_rust(src: &str) -> String {
    let tokens = lex(src);
    let module = parse(tokens).expect("Should parse");
    let hir = lower_module(&module);
    emit_lib(&hir)
}

#[test]
fn codegen_activity_emits_async_fn() {
    let src = r#"
type MyRes = | Ok(v: str) | Error

activity send_email(recipient: str, subject: str) to Result[str]:
    ret Ok(recipient)
"#;
    let output = codegen_rust(src);
    assert!(
        output.contains("pub async fn send_email("),
        "Activity should emit as async fn, got:\n{}",
        output
    );
    assert!(
        output.contains("recipient: "),
        "Should have recipient param"
    );
    assert!(output.contains("subject: "), "Should have subject param");
}

#[test]
fn codegen_with_expression_emits_execute_activity() {
    let src = r#"
type MyRes = | Ok(v: str) | Error

activity fetch_data() to Result[str]:
    ret Ok("data")

workflow main_flow() to Result[str]:
    let res = fetch_data() with { retries: 3, timeout: "10s" }
    ret res
"#;
    let output = codegen_rust(src);
    // The `with` should generate an execute_activity call with options
    assert!(
        output.contains("execute_activity"),
        "Should emit execute_activity call, got:\n{}",
        output
    );
    assert!(
        output.contains("with_retries(3"),
        "Should emit retries option"
    );
    assert!(
        output.contains("parse_duration(\"10s\")"),
        "Should emit timeout as duration parse"
    );
}

#[test]
fn codegen_activity_without_with_is_plain_call() {
    let src = r#"
type MyRes = | Ok(v: str) | Error

activity do_work(input: str) to Result[str]:
    ret Ok(input)

fn main() to Result[str]:
    let result = do_work("test")
    ret result
"#;
    let output = codegen_rust(src);
    // Activity should be defined
    assert!(
        output.contains("pub async fn do_work("),
        "Activity should be emitted"
    );
    // Plain function call (no `with`) should NOT emit execute_activity
    assert!(
        output.contains("do_work("),
        "Should have the call to do_work"
    );
}

#[test]
fn codegen_with_all_options() {
    let src = r#"
fn f() to int:
    let x = 1 with { retries: 5, timeout: "30s", activity_id: "unique-xyz", backoff_multiplier: 2 }
    ret x
"#;
    let output = codegen_rust(src);
    assert!(output.contains("with_retries(5"), "Should emit retries");
    assert!(
        output.contains("parse_duration(\"30s\")"),
        "Should emit timeout"
    );
    assert!(
        output.contains("with_activity_id(\"unique-xyz\""),
        "Should emit activity_id"
    );
    assert!(
        output.contains("with_backoff_multiplier(2"),
        "Should emit backoff_multiplier"
    );
}

// --- Table and Index codegen tests ---

#[test]
fn codegen_table_emits_struct() {
    let src = r#"
@table type Task:
    title: str
    done: bool
    priority: int
"#;
    let output = codegen_rust(src);
    assert!(
        output.contains("pub struct Task"),
        "Should emit struct for @table"
    );
    assert!(
        output.contains("pub _id: Option<i64>"),
        "Should have auto _id field"
    );
    assert!(
        output.contains("pub title: String"),
        "Should have title field"
    );
    assert!(output.contains("pub done: bool"), "Should have done field");
    assert!(
        output.contains("pub priority: i64"),
        "Should have priority field"
    );
}

#[test]
fn codegen_table_emits_ddl() {
    use vox_codegen_rust::emit::emit_table_ddl;

    let src = r#"
@table type Task:
    title: str
    done: bool
    priority: int
"#;
    let tokens = lex(src);
    let module = parse(tokens).expect("Should parse");
    let hir = lower_module(&module);
    assert_eq!(hir.tables.len(), 1, "Should have 1 table");

    let ddl = emit_table_ddl(&hir.tables[0]);
    assert!(
        ddl.contains("CREATE TABLE IF NOT EXISTS task"),
        "DDL should create table 'task'"
    );
    assert!(
        ddl.contains("_id INTEGER PRIMARY KEY AUTOINCREMENT"),
        "DDL should have _id PK"
    );
    assert!(
        ddl.contains("title TEXT NOT NULL"),
        "DDL should have title column"
    );
    assert!(
        ddl.contains("done INTEGER NOT NULL"),
        "DDL: bool maps to INTEGER"
    );
    assert!(
        ddl.contains("priority INTEGER NOT NULL"),
        "DDL: int maps to INTEGER"
    );
}

#[test]
fn codegen_index_emits_ddl() {
    use vox_codegen_rust::emit::emit_index_ddl;

    let src = r#"
@table type Task:
    title: str
    done: bool
    priority: int

@index Task.by_done_priority on (done, priority)
"#;
    let tokens = lex(src);
    let module = parse(tokens).expect("Should parse");
    let hir = lower_module(&module);
    assert_eq!(hir.indexes.len(), 1, "Should have 1 index");

    let ddl = emit_index_ddl(&hir.indexes[0]);
    assert!(
        ddl.contains("CREATE INDEX IF NOT EXISTS idx_task_by_done_priority"),
        "DDL should create index"
    );
    assert!(ddl.contains("ON task"), "DDL should reference table");
    assert!(ddl.contains("(done, priority)"), "DDL should list columns");
}

// --- MCP server codegen tests ---

#[test]
fn codegen_mcp_tool_hir_lowering() {
    let src = r#"
@mcp.tool "Get the weather for a city" fn get_weather(city: str) to str:
    ret city
"#;
    let tokens = lex(src);
    let module = parse(tokens).expect("Should parse");
    let hir = lower_module(&module);

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
@mcp.tool "Get the weather for a city" fn get_weather(city: str) to str:
    ret city
"#;
    let tokens = lex(src);
    let module = parse(tokens).expect("Should parse");
    let hir = lower_module(&module);
    let output = vox_codegen_rust::generate(&hir, "my_mcp_tools").unwrap();

    assert!(
        output.files.contains_key("src/mcp_server.rs"),
        "Should produce mcp_server.rs"
    );

    let mcp = output.files.get("src/mcp_server.rs").unwrap();
    assert!(
        mcp.contains("fn dispatch_tool"),
        "Should have dispatch function"
    );
    assert!(
        mcp.contains("fn tool_list"),
        "Should have tool_list function"
    );
    assert!(mcp.contains("fn main"), "Should have main entry point");
    assert!(
        mcp.contains("\"get_weather\""),
        "Should reference tool name"
    );
    assert!(
        mcp.contains("Get the weather for a city"),
        "Should include description"
    );
    assert!(mcp.contains("\"initialize\""), "Should handle initialize");
    assert!(mcp.contains("\"tools/list\""), "Should handle tools/list");
    assert!(mcp.contains("\"tools/call\""), "Should handle tools/call");
}

#[test]
fn codegen_mcp_server_input_schema() {
    let src = r#"
@mcp.tool "Add two numbers" fn add(a: int, b: int) to int:
    ret a

@mcp.tool "Greet someone" fn greet(name: str) to str:
    ret name
"#;
    let tokens = lex(src);
    let module = parse(tokens).expect("Should parse");
    let hir = lower_module(&module);

    assert_eq!(hir.mcp_tools.len(), 2, "Should have 2 MCP tools");

    let mcp = vox_codegen_rust::emit::emit_mcp_server(&hir, "my_tools");
    // Verify inputSchema for add
    assert!(
        mcp.contains("\"integer\""),
        "int params should map to JSON 'integer' type"
    );
    // Verify inputSchema for greet
    assert!(
        mcp.contains("\"string\""),
        "str params should map to JSON 'string' type"
    );
    // Verify param extraction
    assert!(
        mcp.contains("as_i64"),
        "int params should use as_i64 for extraction"
    );
    assert!(
        mcp.contains("as_str"),
        "str params should use as_str for extraction"
    );
}

#[test]
fn codegen_no_mcp_server_when_no_tools() {
    let src = r#"
fn hello(name: str) to str:
    ret name
"#;
    let tokens = lex(src);
    let module = parse(tokens).expect("Should parse");
    let hir = lower_module(&module);
    let output = vox_codegen_rust::generate(&hir, "test_no_mcp").unwrap();

    assert!(
        !output.files.contains_key("src/mcp_server.rs"),
        "Should NOT produce mcp_server.rs when no @mcp.tool"
    );
}
