use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;
use vox_compiler::typeck::typecheck_ast_module;

fn codes_for(src: &str) -> Vec<String> {
    let m = parse(lex(src)).expect("parse");
    typecheck_ast_module(src, &m)
        .into_iter()
        .filter_map(|d| d.code)
        .collect()
}

#[test]
fn offline_capable_is_surfaced_not_silent() {
    assert!(
        codes_for("@offline_capable\nfn sync_data() { 1 }")
            .iter()
            .any(|c| c == "vox/decorator/offline-capable-unimplemented"),
        "expected vox/decorator/offline-capable-unimplemented diagnostic"
    );
}

#[test]
fn collaborative_is_surfaced_not_silent() {
    assert!(
        codes_for("@collaborative\nfn doc_edit() { 1 }")
            .iter()
            .any(|c| c == "vox/decorator/collaborative-unimplemented"),
        "expected vox/decorator/collaborative-unimplemented diagnostic"
    );
}

#[test]
fn scheduled_on_reactive_fn_is_warned() {
    // @reactive comes between @scheduled(...) and fn — the fn-level decorator loop consumes it.
    let codes = codes_for("@scheduled(\"1h\") @reactive fn tick() { 1 }");
    assert!(
        codes
            .iter()
            .any(|c| c == "vox/decorator/scheduled-target-unsupported"),
        "got {codes:?}"
    );
}

#[test]
fn native_scheduled_is_not_warned() {
    let codes = codes_for("@scheduled(\"1h\")\nfn tick() to Unit { return Unit }");
    assert!(
        !codes
            .iter()
            .any(|c| c == "vox/decorator/scheduled-target-unsupported"),
        "native @scheduled must not trigger scheduled-target-unsupported, got {codes:?}"
    );
}
