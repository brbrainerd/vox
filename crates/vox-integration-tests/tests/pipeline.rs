/// End-to-end integration tests for the Vox compiler pipeline.
/// These tests lex → parse → typecheck → codegen the chatbot example
/// and verify the output is correct.

use vox_lexer::cursor::lex;
use vox_parser::parser::parse;
use vox_typeck::typecheck_module;
use vox_codegen_ts::generate;

const CHATBOT_SRC: &str = r#"import react.use_state

type ChatResult =
    | Ok(text: str)
    | Error(message: str)

@component fn Chat() to Element:
    let (messages, set_messages) = use_state([])
    let (input, set_input) = use_state("")
    let send = fn(msg) set_messages(messages.append({role: "user", text: msg}))
    <div class="chat-container">
        <h1>Vox Chatbot</h1>
        <button on_click={fn(_e) send(input)}>Send</button>
    </div>

http post "/api/chat" to ChatResult:
    let body = request.json()
    let prompt = body.message
    let response = spawn(Claude).send(prompt)
    ret response
"#;

#[test]
fn pipeline_lex_produces_tokens() {
    let tokens = lex(CHATBOT_SRC);
    assert!(tokens.len() > 100, "Expected many tokens, got {}", tokens.len());
}

#[test]
fn pipeline_parse_produces_four_declarations() {
    let tokens = lex(CHATBOT_SRC);
    let module = parse(tokens).expect("Should parse without errors");
    // import, type, component, http route = 4 declarations
    assert_eq!(module.declarations.len(), 4, "import + type + component + route");
}

#[test]
fn pipeline_typecheck_has_no_errors() {
    let tokens = lex(CHATBOT_SRC);
    let module = parse(tokens).unwrap();
    let diagnostics = typecheck_module(&module);
    let errors: Vec<_> = diagnostics.iter()
        .filter(|d| d.severity == vox_typeck::diagnostics::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "Should have no type errors: {:?}", errors);
}

#[test]
fn pipeline_codegen_produces_three_files() {
    let tokens = lex(CHATBOT_SRC);
    let module = parse(tokens).unwrap();
    let output = generate(&module).expect("Should generate without errors");
    assert_eq!(output.files.len(), 3, "types.ts + Chat.tsx + server.ts");

    let filenames: Vec<&str> = output.files.iter().map(|(n, _)| n.as_str()).collect();
    assert!(filenames.contains(&"types.ts"), "Should produce types.ts");
    assert!(filenames.contains(&"Chat.tsx"), "Should produce Chat.tsx");
    assert!(filenames.contains(&"server.ts"), "Should produce server.ts");
}

#[test]
fn codegen_types_has_tagged_union() {
    let tokens = lex(CHATBOT_SRC);
    let module = parse(tokens).unwrap();
    let output = generate(&module).unwrap();

    let types = output.files.iter().find(|(n, _)| n == "types.ts").unwrap();
    assert!(types.1.contains("_tag: \"Ok\""), "Should have Ok tag");
    assert!(types.1.contains("_tag: \"Error\""), "Should have Error tag");
    assert!(types.1.contains("export type ChatResult"), "Should export ChatResult");
}

#[test]
fn codegen_component_has_use_state() {
    let tokens = lex(CHATBOT_SRC);
    let module = parse(tokens).unwrap();
    let output = generate(&module).unwrap();

    let chat = output.files.iter().find(|(n, _)| n == "Chat.tsx").unwrap();
    assert!(chat.1.contains("useState"), "Should use useState hook");
    assert!(chat.1.contains("export function Chat"), "Should export Chat component");
}

#[test]
fn codegen_server_has_express_route_with_await() {
    let tokens = lex(CHATBOT_SRC);
    let module = parse(tokens).unwrap();
    let output = generate(&module).unwrap();

    let server = output.files.iter().find(|(n, _)| n == "server.ts").unwrap();
    assert!(server.1.contains("app.post(\"/api/chat\""), "Should have POST route");
    assert!(server.1.contains("express"), "Should import express");
    assert!(server.1.contains("ClaudeActor"), "Should have Claude actor class");
    assert!(server.1.contains("await new ClaudeActor().send("), "Actor .send() must be awaited");
}

#[test]
fn codegen_jsx_text_content_not_interpolated() {
    let tokens = lex(CHATBOT_SRC);
    let module = parse(tokens).unwrap();
    let output = generate(&module).unwrap();

    let chat = output.files.iter().find(|(n, _)| n == "Chat.tsx").unwrap();
    // Text content like "Vox" and "Chatbot" inside <h1> should appear as plain text,
    // NOT as {Vox} or {Chatbot} JSX expressions
    assert!(!chat.1.contains("{Vox}"), "Bare text should not be wrapped in braces");
    assert!(!chat.1.contains("{Chatbot}"), "Bare text should not be wrapped in braces");
    assert!(!chat.1.contains("{Send}"), "Button text should not be wrapped in braces");
}

// --- TS codegen for activities ---

#[test]
fn codegen_ts_activity_produces_activities_file() {
    let src = r#"
type MyRes = | Ok(v: str) | Error

activity send_email(recipient: str, subject: str) to Result[str]:
    ret Ok(recipient)
"#;
    let tokens = lex(src);
    let module = parse(tokens).unwrap();
    let output = generate(&module).unwrap();

    let filenames: Vec<&str> = output.files.iter().map(|(n, _)| n.as_str()).collect();
    assert!(filenames.contains(&"activities.ts"), "Should produce activities.ts, got: {:?}", filenames);
}

#[test]
fn codegen_ts_activity_has_async_function() {
    let src = r#"
type MyRes = | Ok(v: str) | Error

activity fetch_data(url: str) to Result[str]:
    ret Ok(url)
"#;
    let tokens = lex(src);
    let module = parse(tokens).unwrap();
    let output = generate(&module).unwrap();

    let activities = output.files.iter().find(|(n, _)| n == "activities.ts").unwrap();
    assert!(activities.1.contains("export async function fetch_data("), "Should have async function");
    assert!(activities.1.contains("url: string"), "Should have typed parameter");
    assert!(activities.1.contains("Promise<"), "Should have Promise return type");
}

#[test]
fn codegen_ts_activity_has_runtime_helper() {
    let src = r#"
type MyRes = | Ok(v: str) | Error

activity do_work() to Result[str]:
    ret Ok("done")
"#;
    let tokens = lex(src);
    let module = parse(tokens).unwrap();
    let output = generate(&module).unwrap();

    let activities = output.files.iter().find(|(n, _)| n == "activities.ts").unwrap();
    assert!(activities.1.contains("executeActivity"), "Should include executeActivity helper");
    assert!(activities.1.contains("ActivityOptions"), "Should include ActivityOptions interface");
    assert!(activities.1.contains("parseDuration"), "Should include parseDuration helper");
}

// --- TS codegen for tables ---

#[test]
fn codegen_ts_table_produces_schema_file() {
    let src = r#"
@table type Task:
    title: str
    done: bool
    priority: int
"#;
    let tokens = lex(src);
    let module = parse(tokens).unwrap();
    let output = generate(&module).unwrap();

    let filenames: Vec<&str> = output.files.iter().map(|(n, _)| n.as_str()).collect();
    assert!(filenames.contains(&"schema.ts"), "Should produce schema.ts, got: {:?}", filenames);

    let schema = output.files.iter().find(|(n, _)| n == "schema.ts").unwrap();
    assert!(schema.1.contains("export interface Task {"), "Should have Task interface");
    assert!(schema.1.contains("_id: number"), "Should have _id field");
    assert!(schema.1.contains("title: string"), "Should have title field");
    assert!(schema.1.contains("done: boolean"), "Should have done field");
    assert!(schema.1.contains("priority: number"), "Should have priority field");
}

// --- @v0 codegen tests ---

#[test]
fn codegen_v0_placeholder_from_prompt() {
    let src = r#"@v0 "A stats dashboard with charts" fn Stats() to Element"#;
    let tokens = lex(src);
    let module = parse(tokens).unwrap();
    let output = generate(&module).unwrap();

    let filenames: Vec<&str> = output.files.iter().map(|(n, _)| n.as_str()).collect();
    assert!(filenames.contains(&"Stats.tsx"), "Should produce Stats.tsx, got: {:?}", filenames);

    let stats = output.files.iter().find(|(n, _)| n == "Stats.tsx").unwrap();
    assert!(stats.1.contains("@v0 generated"), "Should contain @v0 marker");
    assert!(stats.1.contains("A stats dashboard with charts"), "Should contain the prompt");
    assert!(stats.1.contains("export function Stats()"), "Should export component function");
}

#[test]
fn codegen_v0_placeholder_from_image() {
    let src = r#"@v0 from "design.png" fn Dashboard() to Element"#;
    let tokens = lex(src);
    let module = parse(tokens).unwrap();
    let output = generate(&module).unwrap();

    let dash = output.files.iter().find(|(n, _)| n == "Dashboard.tsx").unwrap();
    assert!(dash.1.contains("From image: design.png"), "Should reference the image path");
    assert!(dash.1.contains("export function Dashboard()"), "Should export component function");
}

// --- @table / @index end-to-end pipeline tests ---

const DATA_LAYER_SRC: &str = r#"@table type Task:
    title: str
    done: bool
    priority: int

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
    let diagnostics = typecheck_module(&module);
    let errors: Vec<_> = diagnostics.iter()
        .filter(|d| d.severity == vox_typeck::diagnostics::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "Should have no type errors for @table: {:?}", errors);
}

#[test]
fn pipeline_table_hir_lowering() {
    let tokens = lex(DATA_LAYER_SRC);
    let module = parse(tokens).unwrap();
    let hir = vox_hir::lower::lower_module(&module);

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
    let hir = vox_hir::lower::lower_module(&module);
    let output = vox_codegen_rust::generate(&hir, "test_data").unwrap();

    let lib_rs = output.files.get("src/lib.rs").expect("lib.rs");
    assert!(lib_rs.contains("pub struct Task {"), "struct emitted");
    assert!(lib_rs.contains("pub _id: Option<i64>,"), "_id field");
    assert!(lib_rs.contains("pub title: String,"), "title field");

    let main_rs = output.files.get("src/main.rs").expect("main.rs");
    assert!(main_rs.contains("CREATE TABLE IF NOT EXISTS task"), "DDL in main");
    assert!(main_rs.contains("CREATE INDEX IF NOT EXISTS idx_task_by_done"), "index DDL");
    assert!(main_rs.contains("Arc::new(Mutex::new(db))"), "DB wrapped");
}

// --- routes codegen test ---

#[test]
fn codegen_routes_produces_app_tsx() {
    let src = "routes:\n    \"/\" to home\n    \"/about\" to about";
    let tokens = lex(src);
    let module = parse(tokens).unwrap();
    let output = generate(&module).unwrap();

    let filenames: Vec<&str> = output.files.iter().map(|(n, _)| n.as_str()).collect();
    assert!(filenames.contains(&"App.tsx"), "Should produce App.tsx, got: {:?}", filenames);

    let app = output.files.iter().find(|(n, _)| n == "App.tsx").unwrap();
    assert!(app.1.contains("BrowserRouter"), "Should use BrowserRouter");
    assert!(app.1.contains("react-router-dom"), "Should import react-router-dom");
    assert!(app.1.contains("path=\"/\""), "Should have root route");
    assert!(app.1.contains("path=\"/about\""), "Should have about route");
    assert!(app.1.contains("import { home }"), "Should import home component");
    assert!(app.1.contains("import { about }"), "Should import about component");
}

// --- bind={} reactive binding test ---

#[test]
fn codegen_bind_expands_to_value_onchange() {
    let src = r#"@component fn LoginForm() to Element:
    let (email, set_email) = use_state("")
    ret <input bind={email} />"#;
    let tokens = lex(src);
    let module = parse(tokens).unwrap();
    let output = generate(&module).unwrap();

    let component = output.files.iter().find(|(n, _)| n == "LoginForm.tsx").unwrap();
    assert!(component.1.contains("value={email}"), "bind should expand to value prop, got:\n{}", component.1);
    assert!(component.1.contains("onChange="), "bind should expand to onChange handler");
    assert!(component.1.contains("set_email"), "setter should be derived from ident name (set_email)");
    assert!(component.1.contains("e.target.value"), "onChange should use e.target.value");
}

// --- use_effect hook mapping test ---

#[test]
fn codegen_use_effect_maps_to_react_hook() {
    let src = r#"@component fn Timer() to Element:
    let (count, set_count) = use_state(0)
    use_effect(fn(_x) count)
    ret <div>{count}</div>"#;
    let tokens = lex(src);
    let module = parse(tokens).unwrap();
    let output = generate(&module).unwrap();

    let component = output.files.iter().find(|(n, _)| n == "Timer.tsx").unwrap();
    assert!(component.1.contains("useEffect"), "use_effect should map to useEffect, got:\n{}", component.1);
    assert!(component.1.contains("import React, { useState, useEffect }"), "useEffect should be in imports");
}

// --- Phase 5F: Full-stack dashboard integration test ---

#[test]
fn dashboard_full_pipeline_e2e() {
    let src = "type Message = | User(text: str) | Bot(text: str)\n\n@v0 \"A metrics dashboard with KPIs\" fn Dashboard() to Element\n\n@component fn ChatWidget() to Element:\n    let (messages, set_messages) = use_state([])\n    let (input, set_input) = use_state(\"\")\n    ret <div class=\"chat\">\n        <input bind={input} />\n        <button on_click={fn(e) set_input(\"\")} >\"Send\"</button>\n    </div>\n\nhttp get \"/api/stats\" to list[int]:\n    ret 42\n\nroutes:\n    \"/\" to Dashboard\n    \"/chat\" to ChatWidget";

    let tokens = lex(src);
    let module = parse(tokens).unwrap();
    let output = generate(&module).unwrap();

    let filenames: Vec<&str> = output.files.iter().map(|(n, _)| n.as_str()).collect();

    // All expected output files
    assert!(filenames.contains(&"types.ts"), "Should produce types.ts, got: {:?}", filenames);
    assert!(filenames.contains(&"Dashboard.tsx"), "Should produce Dashboard.tsx for @v0");
    assert!(filenames.contains(&"ChatWidget.tsx"), "Should produce ChatWidget.tsx for @component");
    assert!(filenames.contains(&"server.ts"), "Should produce server.ts for @server fn");
    assert!(filenames.contains(&"App.tsx"), "Should produce App.tsx for routes:");

    // @v0 placeholder
    let dash = output.files.iter().find(|(n, _)| n == "Dashboard.tsx").unwrap();
    assert!(dash.1.contains("@v0 generated component"), "Dashboard should be v0 placeholder");
    assert!(dash.1.contains("KPIs"), "Dashboard should contain the prompt text");

    // @component with bind={}
    let chat = output.files.iter().find(|(n, _)| n == "ChatWidget.tsx").unwrap();
    assert!(chat.1.contains("value={input}"), "bind should expand to value");
    assert!(chat.1.contains("onChange="), "bind should expand to onChange");
    assert!(chat.1.contains("set_input"), "bind setter should be set_input");

    // routes -> App.tsx
    let app = output.files.iter().find(|(n, _)| n == "App.tsx").unwrap();
    assert!(app.1.contains("path=\"/\""), "App should route /");
    assert!(app.1.contains("path=\"/chat\""), "App should route /chat");
    assert!(app.1.contains("BrowserRouter"), "App should use BrowserRouter");

    // types.ts
    let types = output.files.iter().find(|(n, _)| n == "types.ts").unwrap();
    assert!(types.1.contains("Message"), "types.ts should contain Message type");

    // server.ts
    let server = output.files.iter().find(|(n, _)| n == "server.ts").unwrap();
    assert!(server.1.contains("/api/stats"), "server.ts should contain /api/stats route");
    assert!(server.1.contains("/api/stats"), "server.ts should contain /api/stats route");
}

#[test]
fn chatbot_full_pipeline_e2e() {
    // This test builds the actual examples/chatbot.vox file
    // We assume the test runner is executed from workspace root or crate root
    // But TestProject usually handles tmp dir.
    // We need to read the file content manually if we use lex/parse directly?
    // Or just use the file path if we had a helper.
    // Since previous tests verify logic using inline strings, we'll read the file content here.

    let path = std::path::Path::new("../../examples/chatbot.vox");
    let src = std::fs::read_to_string(path).unwrap_or_else(|_| {
        // Fallback if running from crate root
        std::fs::read_to_string("examples/chatbot.vox").expect("Could not read examples/chatbot.vox")
    });

    let tokens = vox_lexer::cursor::lex(&src);
    let module = vox_parser::parser::parse(tokens).expect("Chatbot should parse");

    let diagnostics = vox_typeck::typecheck_module(&module);
    let errors: Vec<_> = diagnostics.iter()
        .filter(|d| d.severity == vox_typeck::diagnostics::Severity::Error)
        .collect();
    assert!(errors.is_empty(), "Chatbot should have no type errors: {:?}", errors);

    let output = vox_codegen_ts::generate(&module).expect("Chatbot should generate");

    let filenames: Vec<&str> = output.files.iter().map(|(n, _)| n.as_str()).collect();

    assert!(filenames.contains(&"Chat.tsx"), "Should produce Chat.tsx");
    assert!(filenames.contains(&"Chat.css"), "Should produce Chat.css (from style block)");
    assert!(filenames.contains(&"App.tsx"), "Should produce App.tsx (from routes)");

    let chat_css = output.files.iter().find(|(n, _)| n == "Chat.css").unwrap();
    assert!(chat_css.1.contains(".chat_container"), "CSS should contain .chat_container");

    let chat_tsx = output.files.iter().find(|(n, _)| n == "Chat.tsx").unwrap();
    assert!(chat_tsx.1.contains("import \"./Chat.css\""), "Chat.tsx should import CSS");
    assert!(chat_tsx.1.contains("set_messages"), "Should use set_messages");
}
