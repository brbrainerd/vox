//! Parser integration tests for @server functions.

#[test]
fn parse_server_fn() {
    let source = r#"
@server fn greet(name: str) to str:
    ret "hello"
"#;
    let tokens = vox_lexer::lex(source);
    let module = vox_parser::parser::parse(tokens).expect("should parse @server fn");
    assert_eq!(module.declarations.len(), 1);
    match &module.declarations[0] {
        vox_ast::decl::Decl::ServerFn(sf) => {
            assert_eq!(sf.func.name, "greet");
            assert_eq!(sf.func.params.len(), 1);
            assert_eq!(sf.func.params[0].name, "name");
        }
        other => panic!("Expected ServerFn, got {:?}", other),
    }
}

#[test]
fn parse_server_fn_multiple_params() {
    let source = r#"
@server fn add(a: int, b: int) to int:
    ret a + b
"#;
    let tokens = vox_lexer::lex(source);
    let module = vox_parser::parser::parse(tokens).expect("should parse");
    match &module.declarations[0] {
        vox_ast::decl::Decl::ServerFn(sf) => {
            assert_eq!(sf.func.name, "add");
            assert_eq!(sf.func.params.len(), 2);
        }
        other => panic!("Expected ServerFn, got {:?}", other),
    }
}
