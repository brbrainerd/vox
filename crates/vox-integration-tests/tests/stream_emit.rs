//! Integration tests for stream blocks, emit statements, and Stream<T> typing.
use vox_typeck::{typecheck_hir, env::TypeEnv, builtins::BuiltinTypes};
use vox_hir::lower_module;
use vox_lexer::cursor::lex;
use vox_parser::parser::parse;
use vox_typeck::typecheck_module;

const STREAM_SRC: &str = r#"fn numbers() to int:
    let s = stream:
        emit 1
        emit 2
        emit 3
    ret 0
"#;

// ── Parsing tests ──

#[test]
fn stream_block_parses() {
    let tokens = lex(STREAM_SRC);
    let module = parse(tokens).expect("stream source should parse");
    assert_eq!(module.declarations.len(), 1);
}

// ── HIR lowering tests ──

#[test]
fn stream_lowers_to_hir_stream_block() {
    let tokens = lex(STREAM_SRC);
    let module = parse(tokens).unwrap();
    let hir = lower_module(&module);
    let mut env = TypeEnv::new();
    let builtins = BuiltinTypes::register_all(&mut env);
    let _hir_diags = typecheck_hir(&hir, &mut env, &builtins, "");
    assert_eq!(hir.functions.len(), 1);
    // Check that the body contains a StreamBlock expr (not a plain Block)
    let func = &hir.functions[0];
    let has_stream = func.body.iter().any(|stmt| {
        if let vox_hir::HirStmt::Let { value, .. } = stmt {
            matches!(value, vox_hir::HirExpr::StreamBlock(..))
        } else {
            false
        }
    });
    assert!(has_stream, "Expected HirExpr::StreamBlock in function body");
}

// ── Typeck tests ──

#[test]
fn stream_block_typechecks_without_errors() {
    let tokens = lex(STREAM_SRC);
    let module = parse(tokens).unwrap();
    let diags = typecheck_module(&module, STREAM_SRC);
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
fn emit_outside_stream_reports_error() {
    let src = r#"fn bad() to int:
    emit 42
    ret 0
"#;
    let tokens = lex(src);
    let module = parse(tokens).unwrap();
    let diags = typecheck_module(&module, src);
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| {
            d.severity == vox_typeck::diagnostics::Severity::Error
                && d.message.to_lowercase().contains("emit")
        })
        .collect();
    assert!(
        !errors.is_empty(),
        "Expected an error about emit outside stream, got none"
    );
}
