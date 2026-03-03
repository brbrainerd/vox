use vox_lexer::cursor::lex;
use vox_parser::parser::parse;
use vox_hir::{lower_module, resolve_imports};

#[test]
fn multi_module_import_resolves() {
    let source_a = "pub const FOO = \"A\"\n";
    let source_b = "import a.FOO\n\nfn bar():\n    let x = FOO\n";

    // 1. Lower module A
    let tokens_a = lex(source_a);
    let ast_a = match parse(tokens_a) {
        Ok(m) => m,
        Err(e) => panic!("A parse failed: {:?}", e),
    };
    let mut hir_a = lower_module(&ast_a);
    hir_a.name = "a".to_string();

    // 2. Lower module B
    let tokens_b = lex(source_b);
    let ast_b = match parse(tokens_b) {
        Ok(m) => m,
        Err(e) => panic!("B parse failed: {:?}", e),
    };
    let mut hir_b = lower_module(&ast_b);
    hir_b.name = "b".to_string();

    // 3. Resolve imports
    let modules = vec![hir_a, hir_b];
    let resolved = resolve_imports(&modules);

    // 4. Verify results
    let b_resolves: Vec<_> = resolved.iter()
        .filter(|((mod_name, _), _)| mod_name == "b")
        .collect();

    assert_eq!(b_resolves.len(), 1, "Module 'b' should have 1 resolved import from 'a'");

    let ((mod_name, idx), (target_mod, _target_id)) = b_resolves[0];
    assert_eq!(*mod_name, "b");
    assert_eq!(*target_mod, "a");

    let item = &modules[1].imports[*idx].item;
    assert_eq!(item, "FOO");
}
