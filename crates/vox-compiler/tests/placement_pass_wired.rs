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
