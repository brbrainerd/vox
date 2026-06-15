//! `@deprecated` end-to-end: parsing the optional reason, carrying it through
//! lowering, and surfacing it (and the bare form) in the usage-site warning.
//!
//! Regression guard: the function binding was previously registered with
//! `is_deprecated: false` hardcoded, so the warning never fired for fns.

use vox_compiler::ast::decl::{Decl, FnDecl};
use vox_compiler::hir::lower_module;
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

fn diag_messages(src: &str) -> Vec<String> {
    let m = parse(lex(src)).expect("parse");
    typecheck_ast_module(src, &m)
        .into_iter()
        .map(|d| d.message)
        .collect()
}

fn first_fn(src: &str) -> FnDecl {
    let m = parse(lex(src)).expect("parse");
    m.declarations
        .into_iter()
        .find_map(|d| match d {
            Decl::Function(f) => Some(f),
            _ => None,
        })
        .expect("a function decl")
}

// --- Task 1: activate the warning ------------------------------------------

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

// --- Task 2: parse the optional reason -------------------------------------

#[test]
fn deprecated_reason_parses_and_is_captured() {
    let f = first_fn("@deprecated(\"Use new_function instead\") fn old() to int { return 0 }");
    assert!(f.is_deprecated);
    assert_eq!(
        f.deprecated_reason.as_deref(),
        Some("Use new_function instead")
    );
}

#[test]
fn bare_deprecated_still_parses_with_no_reason() {
    let f = first_fn("@deprecated fn old() to int { return 0 }");
    assert!(f.is_deprecated);
    assert_eq!(f.deprecated_reason, None);
}

// --- Task 3: reason survives lowering --------------------------------------

#[test]
fn deprecated_reason_survives_lowering() {
    let m = parse(lex(
        "@deprecated(\"gone soon\") fn old() to int { return 0 }",
    ))
    .expect("parse");
    let hir = lower_module(&m);
    let f = hir
        .functions
        .iter()
        .find(|f| f.name == "old")
        .expect("lowered fn `old`");
    assert!(f.is_deprecated);
    assert_eq!(f.deprecated_reason.as_deref(), Some("gone soon"));
}

// --- Task 4: surface the reason in the warning -----------------------------

#[test]
fn deprecation_warning_includes_reason() {
    let src = "@deprecated(\"Use new_function instead\") fn old() to int { return 0 }\n\
               fn main() to int { return old() }";
    let msgs = diag_messages(src);
    assert!(
        msgs.iter()
            .any(|m| m.contains("deprecated: Use new_function instead")),
        "expected the reason in the warning message; got {msgs:?}"
    );
}

#[test]
fn bare_deprecation_warning_has_no_reason_suffix() {
    let src = "@deprecated fn old() to int { return 0 }\n\
               fn main() to int { return old() }";
    let msgs = diag_messages(src);
    assert!(
        msgs.iter().any(|m| m == "'old' is deprecated"),
        "bare @deprecated should produce the plain message; got {msgs:?}"
    );
}
