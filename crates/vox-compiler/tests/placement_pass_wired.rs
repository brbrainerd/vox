//! Proves the placement pass runs inside typeck end-to-end.
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;
use vox_compiler::typeck::typecheck_hir_module;

fn codes_for(src: &str) -> Vec<String> {
    let mut hir = lower_module(&parse(lex(src)).expect("parse"));
    typecheck_hir_module(src, &mut hir)
        .into_iter()
        .filter_map(|d| d.code)
        .collect()
}

#[test]
fn gui_calling_native_directly_is_rejected_by_typeck() {
    let codes = codes_for("@reactive fn view() { read_db() }\nfn read_db() uses db { 0 }");
    assert!(
        codes.iter().any(|c| c == "vox/placement/boundary"),
        "expected placement boundary diagnostic; got: {codes:?}"
    );
}

#[test]
fn place_with_unknown_tier_is_a_parse_error_not_silent_shared() {
    // A typo'd placement must be diagnosed, never silently defaulted to Shared.
    let result = parse(lex("@place(bogus) fn f() { 0 }"));
    let errors = result.expect_err("@place(bogus) must be a parse error");
    assert!(
        errors
            .iter()
            .any(|e| format!("{e:?}").contains("placement") || format!("{e:?}").contains("bogus")),
        "error should name the bad placement; got: {errors:?}"
    );
}

#[test]
fn place_with_missing_argument_is_a_parse_error() {
    assert!(
        parse(lex("@place() fn f() { 0 }")).is_err(),
        "@place() with no argument must be a parse error"
    );
}

#[test]
fn place_native_still_parses_clean() {
    parse(lex("@place(native) fn f() { 0 }")).expect("@place(native) must parse cleanly");
}
