#![allow(missing_docs)]

use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;
use vox_compiler::typeck::diagnostics::TypeckSeverity;
use vox_compiler::typeck::typecheck_module;

fn check(src: &str) -> Vec<vox_compiler::typeck::Diagnostic> {
    let tokens = lex(src);
    let module = parse(tokens).expect("Source should parse without errors");
    typecheck_module(&module, "")
}

fn errors(src: &str) -> Vec<vox_compiler::typeck::Diagnostic> {
    check(src)
        .into_iter()
        .filter(|d| d.severity == TypeckSeverity::Error)
        .collect()
}

#[test]
fn test_db_operations_typecheck() {
    // Replaced tombstoned `http post` with a plain `fn` (TASK-2.5).
    let src = r#"
table Message {
    text: str
    timestamp: int
}

fn create_message() to int {
    let msg = {text: "hello", timestamp: 123}
    let id = db.Message.insert(msg)
    match id {
        Ok(_) => 1
        Error(_) => 0
    }
}
"#;

    let errs = errors(src);
    assert!(
        errs.is_empty(),
        "DB operations should typecheck. Errors: {:?}",
        errs
    );
}

#[test]
fn test_db_unknown_table() {
    // Replaced tombstoned `http post` with a plain `fn` (TASK-2.5).
    let src = r#"
fn oops() to Unit {
    let x = db.NonExistentTable
}
"#;
    let errs = errors(src);
    assert!(!errs.is_empty(), "Should error on unknown table");
    assert!(
        errs[0].message.contains("NonExistentTable"),
        "unexpected diagnostic: {:?}",
        errs[0]
    );
}
