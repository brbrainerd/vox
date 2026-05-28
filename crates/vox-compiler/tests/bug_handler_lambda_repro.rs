//! §1.A.1 repro: a `fn()` lambda used as an event-handler attribute emits a never-invoked
//! outer arrow `((...) => (...))` rather than a callable `() => { ... }`.
//!
//! §1.A.3 repro: `.length()` lowers to a method call in TS where `length` is a property.
//!
//! Reference: docs/superpowers/plans/handoff/2026-05-08-handoff-zero-ts-vox-self-sufficient.md §1.A

const FIXTURE_LAMBDA_HANDLER: &str = r#"
import react.use_state

component VoicePage() {
    state status: str = "idle"
    view: column() {
        button(on_click={
            status = "clicked"
        }) { "click me" }
    }
}
"#;

const FIXTURE_MATCH_HANDLER: &str = r#"
import react.use_state

fn make_result() -> Result[str] {
    Ok("hello")
}

component VoicePage() {
    state status: str = "idle"
    view: column() {
        button(on_click={
            let r = make_result()
            match r {
                Ok(t) => { status = t }
                Error(e) => { status = "err" }
            }
        }) { "run" }
        text() { status }
    }
}
"#;

const FIXTURE_LENGTH: &str = r#"
import react.use_state

component LenPage() {
    state label: str = "hello"
    view: column() {
        text() { label.length() }
    }
}
"#;

fn emit(src: &str) -> String {
    let tokens = vox_compiler::lexer::lex(src);
    let module = vox_compiler::parser::parse(tokens).expect("parse");
    let _diags = vox_compiler::typeck::typecheck_module(&module, "handler_test");
    let hir = vox_compiler::hir::lower_module(&module);
    let out = vox_compiler::codegen_ts::generate(&hir).expect("gen");
    out.files.values().map(|b| b.as_str()).collect()
}

/// §1.A.1 — simple assignment handler must be a callable arrow.
///
/// The emitted onClick must be `onClick={() => { set_status("clicked"); }}` or equivalent —
/// NOT a bare expression like `onClick={(() => ...)}` that React never invokes.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn handler_body_is_callable_arrow() {
    let body = emit(FIXTURE_LAMBDA_HANDLER);
    eprintln!("=== handler_body_is_callable_arrow ===\n{body}");
    // The handler value must start with `() =>` or `async () =>` — a function, not a bare expression.
    assert!(
        body.contains("() => {") || body.contains("async () => {"),
        "§1.A.1: onClick handler must be an arrow function, not a bare expression.\nbody:\n{body}"
    );
    // Must not be a never-invoked outer lambda wrapping the real handler.
    // The pattern `((...) => (` followed by `))` without a trailing `()` is the bad shape.
    // Heuristic: the onClick value must not be a pure function-expression statement (no trailing `();` inside the outer arrow).
    assert!(
        !body.contains("((() =>") || body.contains("((() => {"),
        "§1.A.1: emitted a never-invoked IIFE-wrapper `((() => ...))` without `()`. body:\n{body}"
    );
}

/// §1.A.1 — match expression in handler must run when clicked, not wrap in an extra arrow.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn match_handler_emits_invocable_arrow() {
    let body = emit(FIXTURE_MATCH_HANDLER);
    eprintln!("=== match_handler_emits_invocable_arrow ===\n{body}");
    // Handler must be an arrow function containing the match dispatch.
    assert!(
        body.contains("() => {") || body.contains("async () => {"),
        "§1.A.1: match handler must be wrapped in a callable arrow function.\nbody:\n{body}"
    );
    // The match dispatch (switch on _tag or switch(_val)) must appear inside the handler.
    assert!(
        body.contains("switch"),
        "expected match to lower to a switch. body:\n{body}"
    );
}

/// §1.A.3 — `.length()` must lower to a property access `.length`, not a method call.
#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn length_emits_as_property_not_method() {
    let body = emit(FIXTURE_LENGTH);
    eprintln!("=== length_emits_as_property_not_method ===\n{body}");
    assert!(
        !body.contains(".length()"),
        "§1.A.3: `.length()` must not appear in emitted TS — should be `.length`.\nbody:\n{body}"
    );
    assert!(
        body.contains(".length"),
        "§1.A.3: `.length` property access must appear in emitted TS.\nbody:\n{body}"
    );
}
