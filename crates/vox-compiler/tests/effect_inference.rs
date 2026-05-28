//! P1-T6 acceptance: bottom-up effect inference catches transitive violations.
//!
//! The key new behaviour: an unannotated function's inferred effect set is
//! propagated to its callers, so that an annotated caller cannot silently
//! escape its declared effect contract by calling an unannotated helper.

use vox_compiler::hir::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;
use vox_compiler::typeck::effect_check::check_effect_compliance;
use vox_compiler::typeck::{Diagnostic, DiagnosticCategory};

fn check(src: &str) -> Vec<Diagnostic> {
    let m = parse(lex(src)).expect("parse");
    let hir = lower_module(&m);
    check_effect_compliance(&hir, src)
}

fn effect_violations(src: &str) -> Vec<Diagnostic> {
    check(src)
        .into_iter()
        .filter(|d| matches!(d.category, DiagnosticCategory::EffectViolation))
        .collect()
}

/// A `@pure` fn that calls an unannotated helper which itself uses `http.*`
/// must be flagged — even though the helper has no `uses` declaration.
#[test]
fn pure_caller_flagged_when_callee_inferred_to_use_net() {
    let src = r#"
        fn fetch_data(url: str) to str {
            return http.get(url)
        }

        @pure
        fn pure_op(url: str) to str {
            return fetch_data(url)
        }
    "#;
    let ds = effect_violations(src);
    assert!(
        !ds.is_empty(),
        "@pure fn calling net-using helper must emit an effect violation; got: {:?}",
        check(src)
    );
}

/// A function with `uses db` that calls an unannotated helper using `http.*`
/// must be flagged because `net` is not in its declared set.
#[test]
fn db_caller_flagged_when_callee_inferred_to_use_net() {
    let src = r#"
        fn do_http(url: str) to str {
            return http.get(url)
        }

        fn caller(url: str) uses db to str {
            return do_http(url)
        }
    "#;
    let ds = effect_violations(src);
    assert!(
        !ds.is_empty(),
        "fn with `uses db` calling net-using helper must be flagged; got: {:?}",
        check(src)
    );
}

/// An unannotated function calling another unannotated function — no error.
#[test]
fn unannotated_callers_are_open_world() {
    let src = r#"
        fn helper(url: str) to str {
            return http.get(url)
        }

        fn caller(url: str) to str {
            return helper(url)
        }
    "#;
    let ds = effect_violations(src);
    assert!(
        ds.is_empty(),
        "unannotated caller should be open-world and not flagged; got: {ds:?}"
    );
}
