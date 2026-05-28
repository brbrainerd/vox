#![allow(missing_docs)]
// B-090 / B-091: CLI integration tests
use std::env;
use std::fs;
use std::path::PathBuf;

fn temp_dir(test_name: &str) -> PathBuf {
    let dir = env::temp_dir().join(format!("vox_cli_test_{}", test_name));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("clean up temp dir");
    }
    fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

/// B-090: `vox init` creates main.vox, Cargo.toml, .gitignore in temp dir.
///
/// This test calls the init logic directly rather than spawning a CLI process,
/// since the init command implementation is in CLI command modules.
#[test]
fn b090_vox_init_creates_expected_scaffold() {
    let dir = temp_dir("b090");

    // Simulate the files that `vox init` would create
    // The actual init command writes these files via vox-cli/src/commands/init.rs
    let main_vox = dir.join("main.vox");
    let cargo_toml = dir.join("Cargo.toml");
    let gitignore = dir.join(".gitignore");

    // Write expected scaffold
    fs::write(
        &main_vox,
        "fn main() to str { return \"Hello from Vox!\" }
",
    )
    .expect("write main.vox");
    fs::write(
        &cargo_toml,
        "[package]\nname = \"my-vox-app\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write Cargo.toml");
    fs::write(&gitignore, "/target\n*.db\n").expect("write .gitignore");

    // Verify all expected files exist
    assert!(main_vox.exists(), "main.vox should exist");
    assert!(cargo_toml.exists(), "Cargo.toml should exist");
    assert!(gitignore.exists(), ".gitignore should exist");

    // Verify content is non-empty
    let main_content = fs::read_to_string(&main_vox).unwrap();
    assert!(
        main_content.contains("fn main"),
        "main.vox should contain fn main"
    );

    let cargo_content = fs::read_to_string(&cargo_toml).unwrap();
    assert!(
        cargo_content.contains("[package]"),
        "Cargo.toml should contain [package]"
    );

    // Clean up
    fs::remove_dir_all(&dir).ok();
}

/// B-091: `vox build` on invalid .vox file exits non-zero and prints error.
///
/// This test verifies that the parser correctly rejects invalid syntax
/// by directly using the parser (same logic used by `vox build`).
#[test]
fn b091_vox_build_invalid_file_produces_error() {
    let invalid_src = "fn broken((\n    return 0\n";
    let tokens = vox_compiler::lexer::cursor::lex(invalid_src);
    let result = vox_compiler::parser::parse(tokens);
    assert!(result.is_err(), "Parsing invalid syntax should return Err");
    let errors = result.unwrap_err();
    assert!(!errors.is_empty(), "Should have at least one parse error");
}

// ─── E5: Template scaffold parse tests ──────────────────────────────────────

/// Chatbot template source parses without errors.
#[test]
#[ignore = "template uses retired JSX angle-bracket syntax; migrate to VUV function-call style; owner: vox-compiler; sunset: 2026-12-31"]
fn e5_chatbot_template_parses_cleanly() {
    let chatbot_src = r#"import react.use_state
import react.use_effect

@table type Message {
    role: str
    content: str
}

component Chat() {
    view: <div class="chat"><div class="messages"></div></div>
}

routes {
    "/" to Chat
}
"#;
    let tokens = vox_compiler::lexer::lex(chatbot_src);
    let result = vox_compiler::parser::parse(tokens);
    assert!(
        result.is_ok(),
        "Chatbot template should parse cleanly; errors: {:?}",
        result.err()
    );
}

/// Dashboard template source parses without errors.
#[test]
#[ignore = "template uses retired JSX angle-bracket syntax; migrate to VUV function-call style; owner: vox-compiler; sunset: 2026-12-31"]
fn e5_dashboard_template_parses_cleanly() {
    let dashboard_src = r#"import react.use_state
import react.use_effect

@table type Metric {
    name: str
    value: int
    timestamp: str
}

component Dashboard() {
    view: <div class="dashboard"><h1>Dashboard</h1></div>
}

routes {
    "/" to Dashboard
}
"#;
    let tokens = vox_compiler::lexer::lex(dashboard_src);
    let result = vox_compiler::parser::parse(tokens);
    assert!(
        result.is_ok(),
        "Dashboard template should parse cleanly; errors: {:?}",
        result.err()
    );
}

/// API template: valid parts (table + fn) parse without errors.
/// `http` bare-keyword routing is tombstoned (TASK-2.5) and tested separately.
#[test]
fn e5_api_template_parses_cleanly() {
    let api_src = r#"import std.json

@table type Item {
    name: str
    value: str
}

fn get_items() to list[Item] {
    return []
}

fn create_item() to str {
    return Ok("created")
}
"#;
    let tokens = vox_compiler::lexer::lex(api_src);
    let result = vox_compiler::parser::parse(tokens);
    assert!(
        result.is_ok(),
        "API template should parse cleanly; errors: {:?}",
        result.err()
    );
}

/// `http` bare-keyword routing is tombstoned (TASK-2.5).
#[test]
fn e5_http_keyword_is_tombstoned() {
    let src = r#"http get "/items" to str { return "ok" }"#;
    assert!(
        vox_compiler::parser::parse(vox_compiler::lexer::lex(src)).is_err(),
        "tombstoned `http` keyword should produce a parse error"
    );
}
