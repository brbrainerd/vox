use vox_lexer::cursor::lex;
use vox_parser::parser::parse;
use vox_typeck::typecheck_module;
use vox_typeck::diagnostics::Severity;

fn check(src: &str) -> Vec<vox_typeck::Diagnostic> {
    let tokens = lex(src);
    let module = parse(tokens).expect("Source should parse without errors");
    typecheck_module(&module)
}

fn errors(src: &str) -> Vec<vox_typeck::Diagnostic> {
    check(src)
        .into_iter()
        .filter(|d| d.severity == Severity::Error)
        .collect()
}

#[test]
fn test_db_operations_typecheck() {
    let src = r#"
@table type Message:
    text: str
    timestamp: int

http post "/api/msg" to int:
    let msg = {text: "hello", timestamp: 123}
    # Should typecheck: db.Message returns Ty::Table, .insert returns fn(Record)->Result[int]
    let id = db.Message.insert(msg)

    # Check result type (Result[int])
    match id:
        | Ok(i) -> i
        | Error(e) -> 0
"#;

    let errs = errors(src);
    assert!(errs.is_empty(), "DB operations should typecheck. Errors: {:?}", errs);
}

#[test]
fn test_db_unknown_table() {
    let src = r#"
http post "/api/oops" to Unit:
    let x = db.NonExistentTable
"#;
    let errs = errors(src);
    assert!(!errs.is_empty(), "Should error on unknown table");
    assert!(format!("{:?}", errs[0]).contains("Unknown table 'NonExistentTable'"));
}
