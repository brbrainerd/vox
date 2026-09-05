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

/// Two hops: `@pure` → undeclared `middle` → undeclared `leaf` → `http.get`.
/// One-hop inference misses this entirely.
#[test]
fn pure_caller_flagged_across_two_undeclared_hops() {
    let src = r#"
        fn leaf(url: str) to str {
            return http.get(url)
        }

        fn middle(url: str) to str {
            return leaf(url)
        }

        @pure
        fn pure_op(url: str) to str {
            return middle(url)
        }
    "#;
    let ds = effect_violations(src);
    assert!(
        !ds.is_empty(),
        "@pure fn two hops from http.get must be flagged; got: {:?}",
        check(src)
    );
}

/// A named function passed as a value (`xs.map(fetch_one)`) carries its effects.
#[test]
fn named_fn_reference_in_argument_position_carries_effects() {
    let src = r#"
        fn fetch_one(url: str) to str {
            return http.get(url)
        }

        @pure
        fn pure_op(xs: list[str]) to list[str] {
            return xs.map(fetch_one)
        }
    "#;
    let ds = effect_violations(src);
    assert!(
        !ds.is_empty(),
        "passing a net-using fn as a value must be flagged; got: {:?}",
        check(src)
    );
}

/// Mutual recursion must terminate and still propagate the effect.
#[test]
fn mutually_recursive_helpers_terminate_and_propagate() {
    let src = r#"
        fn ping(url: str) to str {
            return pong(url)
        }

        fn pong(url: str) to str {
            return http.get(url)
        }

        fn loopy(url: str) to str {
            return loopy(url)
        }

        @pure
        fn pure_op(url: str) to str {
            return ping(url)
        }
    "#;
    let ds = effect_violations(src);
    assert!(
        !ds.is_empty(),
        "recursive chain reaching http.get must be flagged; got: {:?}",
        check(src)
    );
}

/// A genuinely pure two-hop chain must NOT be flagged (over-reporting guard).
#[test]
fn pure_two_hop_chain_is_clean() {
    let src = r#"
        fn leaf(a: int) to int {
            return a + 1
        }

        fn middle(a: int) to int {
            return leaf(a)
        }

        @pure
        fn pure_op(a: int) to int {
            return middle(a)
        }
    "#;
    let ds = effect_violations(src);
    assert!(ds.is_empty(), "pure chain must not be flagged; got: {ds:?}");
}
