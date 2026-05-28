//! P1-T5 acceptance: non-deterministic calls in `workflow` bodies are flagged.

use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;
use vox_compiler::typeck::typecheck_ast_module;

fn diags(src: &str) -> Vec<vox_compiler::typeck::Diagnostic> {
    let m = parse(lex(src)).expect("parse");
    typecheck_ast_module(src, &m)
}

fn nd_diags(src: &str) -> Vec<vox_compiler::typeck::Diagnostic> {
    diags(src)
        .into_iter()
        .filter(|d| d.code.as_deref() == Some("vox/workflow/non-deterministic-call"))
        .collect()
}

#[test]
fn time_now_in_workflow_is_flagged() {
    let src = r#"
        workflow bad_wf(x: int) to int {
            let t = time.now()
            return x
        }
    "#;
    let ds = nd_diags(src);
    assert!(
        !ds.is_empty(),
        "expected vox/workflow/non-deterministic-call for time.now(); got nothing"
    );
}

#[test]
fn random_in_workflow_is_flagged() {
    let src = r#"
        workflow random_wf(n: int) to int {
            let r = random.int(n)
            return r
        }
    "#;
    let ds = nd_diags(src);
    assert!(
        !ds.is_empty(),
        "expected vox/workflow/non-deterministic-call for random.int(); got nothing"
    );
}

#[test]
fn time_now_in_regular_fn_is_not_flagged() {
    let src = r#"
        fn get_time() to int {
            return time.now()
        }
    "#;
    let ds = nd_diags(src);
    assert!(
        ds.is_empty(),
        "non-deterministic calls in regular fn should not be flagged; got: {ds:?}"
    );
}

#[test]
fn pure_workflow_without_nd_calls_is_clean() {
    let src = r#"
        workflow safe_wf(x: int) to int {
            return x
        }
    "#;
    let ds = nd_diags(src);
    assert!(
        ds.is_empty(),
        "workflow with no nd calls should have no nd diagnostics; got: {ds:?}"
    );
}
