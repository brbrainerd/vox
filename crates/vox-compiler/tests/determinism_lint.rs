//! Task 6.1: determinism lint rejects non-deterministic stdlib calls inside
//! `workflow` bodies (ADR-019 §5). Activities and plain `fn`s are exempt
//! — activities have their return values recorded by the journal; plain
//! fns are not on the durability replay path.

use vox_compiler::typeck::Diagnostic;
use vox_compiler::typeck::typecheck_ast_module;
use vox_compiler::{lexer::cursor::lex, parser::parse};

fn diags(src: &str) -> Vec<Diagnostic> {
    let m = parse(lex(src)).expect("parse");
    typecheck_ast_module(src, &m)
}

fn has_determinism_diag(ds: &[Diagnostic]) -> bool {
    ds.iter()
        .any(|d| d.code.as_deref() == Some("lint.workflow.non_deterministic"))
}

#[test]
fn workflow_using_system_time_is_rejected() {
    let src = r#"
workflow now_capture() to int {
    let t = std.time.now_ms()
    return t
}
"#;
    let ds = diags(src);
    assert!(
        has_determinism_diag(&ds),
        "expected lint.workflow.non_deterministic for std.time.now_ms() inside workflow; got {:?}",
        ds.iter().map(|d| (d.code.clone(), d.message.clone())).collect::<Vec<_>>()
    );
}

#[test]
fn workflow_using_random_is_rejected() {
    let src = r#"
workflow random_pick() to int {
    let r = std.random()
    return r
}
"#;
    let ds = diags(src);
    assert!(
        has_determinism_diag(&ds),
        "expected lint.workflow.non_deterministic for std.random() inside workflow; got {:?}",
        ds.iter().map(|d| (d.code.clone(), d.message.clone())).collect::<Vec<_>>()
    );
}

#[test]
fn activity_using_system_time_is_allowed() {
    let src = r#"
activity now_capture() to int {
    let t = std.time.now_ms()
    return t
}
"#;
    let ds = diags(src);
    assert!(
        !has_determinism_diag(&ds),
        "activity may use system time; got determinism diag(s): {:?}",
        ds.iter()
            .filter(|d| d.code.as_deref() == Some("lint.workflow.non_deterministic"))
            .map(|d| d.message.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn plain_fn_using_system_time_is_allowed() {
    let src = r#"
fn now_capture() to int {
    let t = std.time.now_ms()
    return t
}
"#;
    let ds = diags(src);
    assert!(
        !has_determinism_diag(&ds),
        "plain fn may use system time; got determinism diag(s): {:?}",
        ds.iter()
            .filter(|d| d.code.as_deref() == Some("lint.workflow.non_deterministic"))
            .map(|d| d.message.clone())
            .collect::<Vec<_>>()
    );
}
