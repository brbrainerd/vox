//! `@deprecated fn` must emit a usage-site deprecation warning.
//! Regression guard: the function binding was previously registered with
//! `is_deprecated: false` hardcoded, so the warning never fired for fns.

use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;
use vox_compiler::typeck::typecheck_ast_module;

fn diag_codes(src: &str) -> Vec<String> {
    let m = parse(lex(src)).expect("parse");
    typecheck_ast_module(src, &m)
        .into_iter()
        .filter_map(|d| d.code)
        .collect()
}

#[test]
fn deprecated_fn_use_warns() {
    let src = "@deprecated fn old() to int { return 0 }\n\
               fn main() to int { return old() }";
    let codes = diag_codes(src);
    assert!(
        codes.iter().any(|c| c == "typecheck.deprecated_ident"),
        "expected a deprecation warning on use of `old`; got {codes:?}"
    );
}
