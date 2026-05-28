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

// M-6: transitive determinism — workflow → plain helper fn that calls
// a non-deterministic primitive is just as broken as inlining the call.

#[test]
fn workflow_calling_plain_fn_using_system_time_is_rejected() {
    let src = r#"
fn helper() to int {
    return std.time.now_ms()
}

workflow w() to int {
    return helper()
}
"#;
    let ds = diags(src);
    assert!(
        has_determinism_diag(&ds),
        "workflow that calls a plain fn invoking std.time.now_ms() must \
         be rejected transitively; got diags: {:?}",
        ds.iter().map(|d| (d.code.clone(), d.message.clone())).collect::<Vec<_>>()
    );
}

#[test]
fn workflow_calling_activity_using_system_time_is_allowed() {
    // Activities journal their result — replay returns the recorded
    // value, so internal non-determinism is replay-safe and the lint
    // must NOT fire transitively through an activity boundary.
    let src = r#"
activity stamped() to int {
    return std.time.now_ms()
}

workflow w() to int {
    return stamped()
}
"#;
    let ds = diags(src);
    assert!(
        !has_determinism_diag(&ds),
        "workflow → activity → non-det should be allowed (journal records \
         the activity's return value); got diags: {:?}",
        ds.iter()
            .filter(|d| d.code.as_deref() == Some("lint.workflow.non_deterministic"))
            .map(|d| d.message.clone())
            .collect::<Vec<_>>()
    );
}

#[test]
fn workflow_transitive_chain_through_two_plain_fns_is_rejected() {
    // workflow → outer() → inner() → std.time.now_ms() — multi-hop
    // transitive walk. (Bare single-letter names like `a` collide with
    // JSX elements in Vox's type system, hence the longer names.)
    let src = r#"
fn inner() to int {
    return std.time.now_ms()
}

fn outer() to int {
    return inner()
}

workflow w() to int {
    return outer()
}
"#;
    let ds = diags(src);
    assert!(
        has_determinism_diag(&ds),
        "multi-hop transitive workflow→outer→inner→std.time.now_ms() must be rejected; got diags: {:?}",
        ds.iter().map(|d| (d.code.clone(), d.message.clone())).collect::<Vec<_>>()
    );
}

#[test]
fn transitive_walk_terminates_on_cycle() {
    // Mutually-recursive plain fns: ping() → pong() → ping() → ... —
    // the visited set must break the cycle. The eventual non-det call
    // inside `pong` should still be detected.
    let src = r#"
fn ping() to int {
    if true {
        return pong()
    } else {
        return 0
    }
}

fn pong() to int {
    if true {
        return std.time.now_ms()
    } else {
        return ping()
    }
}

workflow w() to int {
    return ping()
}
"#;
    let ds = diags(src);
    assert!(
        has_determinism_diag(&ds),
        "mutually-recursive helpers must still surface the non-det call \
         without infinite-looping; got diags: {:?}",
        ds.iter().map(|d| (d.code.clone(), d.message.clone())).collect::<Vec<_>>()
    );
}
