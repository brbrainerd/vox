use vox_hir::lower_module;
use vox_parser::parser::parse;
use vox_lexer::lex;
use vox_typeck::{env::TypeEnv, builtins::BuiltinTypes, typecheck_hir};

fn check_hir(source: &str) -> Vec<vox_typeck::Diagnostic> {
    let tokens = lex(source);
    let module = parse(tokens).expect("Should parse");
    let hir = lower_module(&module);
    let mut env = TypeEnv::new();
    let builtins = BuiltinTypes::register_all(&mut env);
    typecheck_hir(&hir, &mut env, &builtins, source)
}

#[test]
fn test_hir_checker_basic_func() {
    let diags = check_hir("fn foo() -> int { 5 }");
    assert!(diags.is_empty(), "Should have no diags: {:?}", diags);
}

#[test]
fn test_hir_checker_return_mismatch() {
    let diags = check_hir("fn foo() -> int { \"hello\" }");
    assert!(!diags.is_empty(), "Expected error diagnostic, got none");
}

#[test]
fn test_hir_checker_actor_handler() {
    let src = "
    actor MyActor {
        on Update(x: int) {
            let y: int = x;
        }
    }
    ";
    let diags = check_hir(src);
    assert!(diags.is_empty(), "{:?}", diags);
}

#[test]
fn test_hir_checker_list_comprehension() {
    let src = "
    fn test() {
        let list = [1, 2, 3];
        let squares = [x * x for x in list if x > 1];
    }
    ";
    let diags = check_hir(src);
    assert!(diags.is_empty(), "{:?}", diags);
}

#[test]
fn test_hir_checker_pattern_match() {
    let src = "
    fn opt_unwrap(x: Option<int>) -> int {
        match x {
            Some(v) => v,
            None => 0
        }
    }
    ";
    let diags = check_hir(src);
    assert!(diags.is_empty(), "{:?}", diags);
}
