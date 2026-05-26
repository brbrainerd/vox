use vox_compiler::typeck::typecheck_ast_module;
use vox_compiler::{lexer::cursor::lex, parser::parse};

#[test]
fn async_handler_with_setstate_warns() {
    // Handler calls an @endpoint fn and then assigns to a state variable.
    // The lint should fire because the lambda is not @cancellable.
    let src = r#"
@query fn slow_fetch() to int { return 1 }
component C() {
    state n: int = 0
    view: column(raw_class="c") {
        button(raw_class="go", on_click=fn(_e) {
            let x = slow_fetch()
            n = x
        }) { "Go" }
    }
}
"#;
    let m = parse(lex(src)).expect("parse");
    let ds = typecheck_ast_module(src, &m);
    let hit = ds
        .iter()
        .find(|d| d.code.as_deref() == Some("lint.handler.uncancellable_async"));
    assert!(
        hit.is_some(),
        "expected lint.handler.uncancellable_async; got {:?}",
        ds.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
}

#[test]
fn cancellable_handler_passes() {
    // Same handler but annotated @cancellable — lint should be silent.
    let src = r#"
@query fn slow_fetch() to int { return 1 }
component C() {
    state n: int = 0
    view: column(raw_class="c") {
        button(raw_class="go", on_click=@cancellable fn(_e) {
            let x = slow_fetch()
            n = x
        }) { "Go" }
    }
}
"#;
    let m = parse(lex(src)).expect("parse");
    let ds = typecheck_ast_module(src, &m);
    assert!(
        ds.iter()
            .all(|d| d.code.as_deref() != Some("lint.handler.uncancellable_async")),
        "@cancellable should silence the lint"
    );
}

#[test]
fn pure_sync_handler_does_not_warn() {
    // Handler only does state assignment — no endpoint call — should not warn.
    let src = r#"
component C() {
    state n: int = 0
    view: column(raw_class="c") {
        button(raw_class="inc", on_click=fn(_e) {
            n = n + 1
        }) { "+" }
    }
}
"#;
    let m = parse(lex(src)).expect("parse");
    let ds = typecheck_ast_module(src, &m);
    assert!(
        ds.iter()
            .all(|d| d.code.as_deref() != Some("lint.handler.uncancellable_async")),
        "sync handler should not warn"
    );
}
