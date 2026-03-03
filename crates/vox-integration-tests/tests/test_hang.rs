use vox_lexer::cursor::lex;
use vox_parser::parser::parse;
use vox_typeck::typecheck_module;

#[test]
fn test_hang() {
    let src = "message ChatMessage:\n    sender: str\n    text: str\n    timestamp: int\n";
    println!("Lexing...");
    let tokens = lex(src);
    for t in &tokens {
        println!("{:?}", t.token);
    }
    println!("Parsing...");
    let module = parse(tokens).unwrap();
    println!("Parsed AST: {:?}", module.declarations);
    println!("Typechecking...");
    let _diags = typecheck_module(&module, src);
    println!("Done");
}
