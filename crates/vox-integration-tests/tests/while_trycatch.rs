//! Integration tests for while, break, continue, try/catch constructs.
use vox_typeck::{typecheck_hir, env::TypeEnv, builtins::BuiltinTypes};
use vox_hir::lower_module;
use vox_lexer::cursor::lex;
use vox_parser::parser::parse;
use vox_typeck::typecheck_module;

const WHILE_SRC: &str = r#"fn count_to_ten() to int:
    let mut i = 0
    while i < 10:
        i = i + 1
    ret i
"#;

const BREAK_SRC: &str = r#"fn find_first() to int:
    let mut result = 0
    let mut i = 0
    while i < 100:
        if i is 42:
            result = i
            break
        i = i + 1
    ret result
"#;

const TRY_CATCH_SRC: &str = r#"fn safe_divide(a: int, b: int) to str:
    try:
        let result = a / b
        ret "ok"
    catch e:
        ret "error"
    ret ""
"#;

// ── Parsing tests ──

#[test]
fn while_parses_correctly() {
    let tokens = lex(WHILE_SRC);
    let module = parse(tokens).expect("while source should parse");
    assert_eq!(module.declarations.len(), 1, "Expected 1 declaration");
}

#[test]
fn break_and_continue_parse_correctly() {
    let tokens = lex(BREAK_SRC);
    let module = parse(tokens).expect("break source should parse");
    assert_eq!(module.declarations.len(), 1, "Expected 1 declaration");
}

#[test]
fn try_catch_parses_correctly() {
    let tokens = lex(TRY_CATCH_SRC);
    let module = parse(tokens).expect("try/catch source should parse");
    assert_eq!(module.declarations.len(), 1, "Expected 1 declaration");
}

// ── HIR lowering tests ──

#[test]
fn while_lowers_to_hir_while() {
    let tokens = lex(WHILE_SRC);
    let module = parse(tokens).unwrap();
    let hir = lower_module(&module);
    let mut env = TypeEnv::new();
    let builtins = BuiltinTypes::register_all(&mut env);
    let _hir_diags = typecheck_hir(&hir, &mut env, &builtins, "");
    assert_eq!(hir.functions.len(), 1);
    // Verify the function body contains a While expression (not Block(If(...)))
    let func = &hir.functions[0];
    let has_while = func.body.iter().any(|stmt| {
        if let vox_hir::HirStmt::Expr { expr, .. } = stmt {
            matches!(expr, vox_hir::HirExpr::While { .. })
        } else {
            false
        }
    });
    assert!(
        has_while,
        "Expected HirExpr::While in function body, but it was not found"
    );
}

#[test]
fn try_catch_lowers_to_hir_try_catch() {
    let tokens = lex(TRY_CATCH_SRC);
    let module = parse(tokens).unwrap();
    let hir = lower_module(&module);
    let mut env = TypeEnv::new();
    let builtins = BuiltinTypes::register_all(&mut env);
    let _hir_diags = typecheck_hir(&hir, &mut env, &builtins, "");
    assert_eq!(hir.functions.len(), 1);
    let func = &hir.functions[0];
    let has_try_catch = func.body.iter().any(|stmt| {
        if let vox_hir::HirStmt::Expr { expr, .. } = stmt {
            matches!(expr, vox_hir::HirExpr::TryCatch { .. })
        } else {
            false
        }
    });
    assert!(has_try_catch, "Expected HirExpr::TryCatch in function body");
}

// ── Typeck tests ──

#[test]
fn while_typechecks_without_errors() {
    let tokens = lex(WHILE_SRC);
    let module = parse(tokens).unwrap();
    let diags = typecheck_module(&module, WHILE_SRC);
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
fn try_catch_typechecks_without_errors() {
    let tokens = lex(TRY_CATCH_SRC);
    let module = parse(tokens).unwrap();
    let diags = typecheck_module(&module, TRY_CATCH_SRC);
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
