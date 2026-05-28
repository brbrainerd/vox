//! P1-T7 acceptance: `side_effect { … }` blocks for sanctioned non-determinism.

use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;
use vox_compiler::typeck::typecheck_ast_module;

fn nd_diags(src: &str) -> Vec<vox_compiler::typeck::Diagnostic> {
    typecheck_ast_module(src, &parse(lex(src)).expect("parse"))
        .into_iter()
        .filter(|d| d.code.as_deref() == Some("vox/workflow/non-deterministic-call"))
        .collect()
}

fn outside_wf_diags(src: &str) -> Vec<vox_compiler::typeck::Diagnostic> {
    typecheck_ast_module(src, &parse(lex(src)).expect("parse"))
        .into_iter()
        .filter(|d| d.code.as_deref() == Some("vox/workflow/side-effect-outside-workflow"))
        .collect()
}

/// `side_effect { … }` must parse without error inside a workflow body.
#[test]
fn side_effect_block_parses() {
    let src = r#"
        workflow proc() to int {
            return side_effect { time.now() }
        }
    "#;
    let m = parse(lex(src)).expect("parse");
    let diags = typecheck_ast_module(src, &m);
    let parse_errs: Vec<_> = diags
        .iter()
        .filter(|d| d.code.as_deref() == Some("E_PARSE"))
        .collect();
    assert!(
        parse_errs.is_empty(),
        "should parse without error: {parse_errs:?}"
    );
}

/// `side_effect { }` suppresses the `vox/workflow/non-deterministic-call` diagnostic.
#[test]
fn side_effect_suppresses_non_determinism_check() {
    let src = r#"
        workflow proc() to int {
            return side_effect { time.now() }
        }
    "#;
    let ds = nd_diags(src);
    assert!(
        ds.is_empty(),
        "side_effect should suppress determinism check; got {ds:?}"
    );
}

/// `side_effect` outside a workflow body must emit `vox/workflow/side-effect-outside-workflow`.
#[test]
fn side_effect_outside_workflow_is_an_error() {
    let src = r#"
        fn plain() to int {
            return side_effect { time.now() }
        }
    "#;
    let ds = outside_wf_diags(src);
    assert!(
        !ds.is_empty(),
        "expected side-effect-outside-workflow diagnostic; got nothing"
    );
}

/// Two `side_effect` blocks in the same workflow must produce two distinct synthesised activities.
#[test]
fn nested_side_effect_block_creates_distinct_activities() {
    let src = r#"
        workflow proc() to int {
            let a = side_effect { time.now() }
            let b = side_effect { time.now() }
            return a + b
        }
    "#;
    let m = parse(lex(src)).expect("parse");
    let hir = lower_module(&m);
    let synthesised: Vec<_> = hir
        .functions
        .iter()
        .filter(|f| f.name.starts_with("__side_effect_"))
        .collect();
    assert_eq!(
        synthesised.len(),
        2,
        "two side_effect blocks → two synthesised activities; got: {:?}",
        synthesised.iter().map(|f| &f.name).collect::<Vec<_>>()
    );
    assert_ne!(synthesised[0].name, synthesised[1].name);
}
