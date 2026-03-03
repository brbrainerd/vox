//! Integration tests for agent and message declarations.
use vox_typeck::{typecheck_hir, env::TypeEnv, builtins::BuiltinTypes};
use vox_hir::lower_module;
use vox_lexer::cursor::lex;
use vox_parser::parser::parse;
use vox_typeck::typecheck_module;

const AGENT_SRC: &str = r#"agent ChatAgent:
    history: str
    count: int
    on handle_msg(text: str) to str:
        ret text
"#;

const MESSAGE_SRC: &str = r#"message ChatMessage:
    sender: str
    text: str
    timestamp: int
"#;

const COMBINED_SRC: &str = r#"message UserInput:
    text: str

agent EchoAgent:
    last_input: str
    on receive(msg: str) to str:
        ret msg
"#;

// ── Parsing tests ──

#[test]
fn agent_decl_parses() {
    let tokens = lex(AGENT_SRC);
    let module = parse(tokens).expect("agent source should parse");
    assert_eq!(module.declarations.len(), 1, "Expected 1 declaration");
    match &module.declarations[0] {
        vox_ast::decl::Decl::Agent(a) => {
            assert_eq!(a.name, "ChatAgent");
            assert_eq!(a.state_fields.len(), 2);
            assert_eq!(a.handlers.len(), 1);
            assert_eq!(a.handlers[0].event_name, "handle_msg");
        }
        other => panic!("Expected Agent declaration, got {:?}", other),
    }
}

#[test]
fn message_decl_parses() {
    let tokens = lex(MESSAGE_SRC);
    let module = parse(tokens).expect("message source should parse");
    assert_eq!(module.declarations.len(), 1, "Expected 1 declaration");
    match &module.declarations[0] {
        vox_ast::decl::Decl::Message(m) => {
            assert_eq!(m.name, "ChatMessage");
            assert_eq!(m.fields.len(), 3);
        }
        other => panic!("Expected Message declaration, got {:?}", other),
    }
}

// ── HIR lowering tests ──

#[test]
fn agent_lowers_to_hir() {
    let tokens = lex(AGENT_SRC);
    let module = parse(tokens).unwrap();
    let hir = lower_module(&module);
    let mut env = TypeEnv::new();
    let builtins = BuiltinTypes::register_all(&mut env);
    let _hir_diags = typecheck_hir(&hir, &mut env, &builtins, "");
    assert_eq!(hir.native_agents.len(), 1);
    let agent = &hir.native_agents[0];
    assert_eq!(agent.name, "ChatAgent");
    assert_eq!(agent.state_fields.len(), 2);
    assert_eq!(agent.handlers.len(), 1);
}

#[test]
fn message_lowers_to_hir() {
    let tokens = lex(MESSAGE_SRC);
    let module = parse(tokens).unwrap();
    let hir = lower_module(&module);
    let mut env = TypeEnv::new();
    let builtins = BuiltinTypes::register_all(&mut env);
    let _hir_diags = typecheck_hir(&hir, &mut env, &builtins, "");
    assert_eq!(hir.messages.len(), 1);
    let msg = &hir.messages[0];
    assert_eq!(msg.name, "ChatMessage");
    assert_eq!(msg.fields.len(), 3);
}

// ── Typeck tests ──

#[test]
fn agent_typechecks_without_errors() {
    let tokens = lex(AGENT_SRC);
    let module = parse(tokens).unwrap();
    let diags = typecheck_module(&module, AGENT_SRC);
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.severity == vox_typeck::diagnostics::Severity::Error)
        .collect();
    assert!(
        errors.is_empty(),
        "Expected no type errors, got: {:?}",
        errors
    );
}

#[test]
fn message_typechecks_without_errors() {
    let tokens = lex(MESSAGE_SRC);
    let module = parse(tokens).unwrap();
    let diags = typecheck_module(&module, MESSAGE_SRC);
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.severity == vox_typeck::diagnostics::Severity::Error)
        .collect();
    assert!(
        errors.is_empty(),
        "Expected no type errors, got: {:?}",
        errors
    );
}

#[test]
fn combined_agent_message_typechecks() {
    let tokens = lex(COMBINED_SRC);
    let module = parse(tokens).unwrap();
    let diags = typecheck_module(&module, COMBINED_SRC);
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.severity == vox_typeck::diagnostics::Severity::Error)
        .collect();
    assert!(
        errors.is_empty(),
        "Expected no type errors, got: {:?}",
        errors
    );
}

// ── Codegen tests ──

#[test]
fn agent_rust_codegen_produces_struct() {
    let tokens = lex(AGENT_SRC);
    let module = parse(tokens).unwrap();
    let hir = lower_module(&module);
    let mut env = TypeEnv::new();
    let builtins = BuiltinTypes::register_all(&mut env);
    let _hir_diags = typecheck_hir(&hir, &mut env, &builtins, "");
    let output = vox_codegen_rust::generate(&hir, "test_pkg").unwrap();
    let lib_content = output.files.get("src/lib.rs").expect("lib.rs should exist");
    assert!(
        lib_content.contains("pub struct ChatAgent"),
        "Expected struct ChatAgent in output"
    );
    assert!(
        lib_content.contains("pub async fn handle_msg"),
        "Expected handler method in output"
    );
}

#[test]
fn message_rust_codegen_produces_struct() {
    let tokens = lex(MESSAGE_SRC);
    let module = parse(tokens).unwrap();
    let hir = lower_module(&module);
    let mut env = TypeEnv::new();
    let builtins = BuiltinTypes::register_all(&mut env);
    let _hir_diags = typecheck_hir(&hir, &mut env, &builtins, "");
    let output = vox_codegen_rust::generate(&hir, "test_pkg").unwrap();
    let lib_content = output.files.get("src/lib.rs").expect("lib.rs should exist");
    assert!(
        lib_content.contains("pub struct ChatMessage"),
        "Expected struct ChatMessage in output"
    );
    assert!(
        lib_content.contains("pub sender:"),
        "Expected sender field in output"
    );
}

#[test]
fn agent_ts_codegen_produces_class() {
    let tokens = lex(AGENT_SRC);
    let module = parse(tokens).unwrap();
    let output = vox_codegen_ts::generate(&module).unwrap();
    let agent_file = output.files.iter().find(|(name, _)| name == "ChatAgent.ts");
    assert!(agent_file.is_some(), "Expected ChatAgent.ts in output");
    let (_, content) = agent_file.unwrap();
    assert!(
        content.contains("export class ChatAgent"),
        "Expected class ChatAgent"
    );
    assert!(
        content.contains("async handle_msg"),
        "Expected async handle_msg handler"
    );
}

#[test]
fn message_ts_codegen_produces_interface() {
    let tokens = lex(MESSAGE_SRC);
    let module = parse(tokens).unwrap();
    let output = vox_codegen_ts::generate(&module).unwrap();
    let msg_file = output.files.iter().find(|(name, _)| name == "messages.ts");
    assert!(msg_file.is_some(), "Expected messages.ts in output");
    let (_, content) = msg_file.unwrap();
    assert!(
        content.contains("export interface ChatMessage"),
        "Expected interface ChatMessage"
    );
}
