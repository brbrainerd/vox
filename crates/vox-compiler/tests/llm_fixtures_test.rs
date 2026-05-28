use std::fs;
use vox_compiler::hir::lower::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

#[test]
fn test_all_llm_fixtures() {
    let dir = "tests/llm_fixtures";
    let entries = match fs::read_dir(dir) {
        Ok(dir) => dir,
        Err(_) => return, // No fixtures yet
    };
    for entry in entries {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("vox") {
            let source = fs::read_to_string(&path).unwrap();
            let tokens = lex(&source);
            let ast = parse(tokens).unwrap_or_else(|_| panic!("Parse error in {:?}", path));
            let _hir = lower_module(&ast);
        }
    }
}
