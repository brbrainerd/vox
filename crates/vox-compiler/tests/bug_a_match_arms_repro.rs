//! Bug A repro: `match` arms emit `case _:` literal patterns and lose Ok/Error bindings,
//! per docs/superpowers/plans/language/2026-05-08-codegen-ts-bugs-blocking-tracker.md.

const FIXTURE: &str = r#"
import react.use_state

fn produce() -> Result[str] {
    Ok("hi")
}

component VoicePage() {
    state status: str = ""
    view: column() {
        button(on_click={
            let r = produce()
            match r {
                Ok(t) => { status = t }
                Error(e) => { status = "fail: " + e }
            }
            let _ = Ok("forced_ctor_emit")
        }) { "go" }
        text() { status }
    }
}
"#;

#[test]
#[ignore = "owner: platform-ci — sunset: 2026-08-01 — compiler test baseline; safety burndown"]
fn match_result_arms_emit_tagged_union_dispatch() {
    let tokens = vox_compiler::lexer::lex(FIXTURE);
    let module = vox_compiler::parser::parse(tokens).expect("parse");
    let _diags = vox_compiler::typeck::typecheck_module(&module, "bug_a_match");
    let hir = vox_compiler::hir::lower_module(&module);
    let out = vox_compiler::codegen_ts::generate(&hir).expect("gen");
    let body: String = out.files.values().map(|b| b.as_str()).collect();
    eprintln!("=== files ===");
    for (n, b) in &out.files {
        if b.contains("case _") || n.contains("VoicePage") {
            eprintln!("--- {n} ---\n{b}\n");
        } else {
            eprintln!("(skip {n})");
        }
    }
    let case_underscore_count = body.matches("case _:").count();
    assert_eq!(
        case_underscore_count, 0,
        "Bug A: match arms emit `case _:` literal patterns. body:\n{body}"
    );
    // Constructor patterns must dispatch on `_tag` (consistent with adt.rs codegen).
    assert!(
        body.contains("_tag") || !body.contains("switch"),
        "expected `_tag`-discriminated dispatch for Result match. body:\n{body}"
    );
    // Bound variables must appear before they're used.
    assert!(
        !body.contains("set_status(t)") || body.contains("const t ="),
        "expected `t` to be bound before `set_status(t)`. body:\n{body}"
    );
}
