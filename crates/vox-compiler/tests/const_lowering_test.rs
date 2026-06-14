/// The parser turns top-level `let` into `Decl::Const`; verify the HIR lowering routes
/// it into `HirModule::consts` (not the `legacy_ast_nodes` catch-all).
#[test]
fn top_level_let_named_max_retries_lowers_to_hir_consts() {
    let tokens = vox_compiler::lexer::lex("let MAX_RETRIES = 3");
    let module = vox_compiler::parser::descent::parse(tokens).expect("parse");
    let hir = vox_compiler::hir::lower::lower_module(&module);
    assert_eq!(hir.consts.len(), 1, "const must produce a HirConst");
    assert_eq!(hir.consts[0].name, "MAX_RETRIES");
    assert!(
        hir.legacy_ast_nodes.is_empty(),
        "const must not fall into legacy_ast_nodes"
    );
}

#[test]
fn top_level_let_lowers_to_hir_consts() {
    // parser emits Decl::Const for a top-level `let` (descent/mod.rs:627)
    let tokens = vox_compiler::lexer::lex(r#"let base_url = "https://api.example.com""#);
    let module = vox_compiler::parser::descent::parse(tokens).expect("parse");
    let hir = vox_compiler::hir::lower::lower_module(&module);
    assert_eq!(hir.consts.len(), 1);
    assert_eq!(hir.consts[0].name, "base_url");
}
