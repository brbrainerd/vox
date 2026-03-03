use vox_hir::lower_module;

fn main() {
    let source = "@theme default:\n    light:\n        bg: \"#ffffff\"\n    dark:\n        bg: \"#000000\"\n";
    let tokens = vox_lexer::lex(source);
    let module = vox_parser::parse(tokens).expect("Parse failed");
    let hir = lower_module(&module);
    println!("Found {} themes", hir.themes.len());
    assert_eq!(hir.themes.len(), 1);
    assert_eq!(hir.themes[0].name, "default");
    println!("Success!");
}
