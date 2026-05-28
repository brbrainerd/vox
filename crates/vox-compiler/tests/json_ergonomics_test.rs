//! Integration test for the strict-Option Json API
//! (json-ergonomics-rfc-2026-05-23).
//!
//! Covers the four invariants the RFC promises:
//!
//! 1. `json.parse(s)` returns `Result[Json]`; both Ok and Err paths surface.
//! 2. Navigation methods (`get`, `at`, `pointer`) return `Option[Json]` and
//!    yield `None` on miss / wrong-receiver-type (no silent Null propagation).
//! 3. Leaf coercion (`as_str`, `as_int`, `as_float`, `as_bool`) returns
//!    `Option[T]` and yields `None` on wrong-type / null.
//! 4. `has` / `is_null` give bool answers without unwrapping.
//!
//! Without this test the API depends on the script corpus to catch
//! regressions, which is too coarse — a change to e.g. the navigation
//! semantics could silently break the no-silent-Null contract.

use vox_compiler::eval::value::VoxValue;
use vox_compiler::eval::Interpreter;
use vox_compiler::hir::lower::lower_module;
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse_script;

/// Lower and run a script; return whatever main() returned, or the formatted
/// error.
fn run(src: &str) -> Result<VoxValue, String> {
    let tokens = lex(src);
    let module = parse_script(tokens).map_err(|errs| format!("parse: {} errors", errs.len()))?;
    let lowered = lower_module(&module);
    let mut interp = Interpreter::new(10_000_000);
    interp.run_module(&lowered).map_err(|e| format!("module: {e:?}"))?;
    interp.call("main", vec![]).map_err(|e| format!("main: {e:?}"))
}

fn run_expect_int(src: &str) -> i64 {
    match run(src).expect("run") {
        VoxValue::Int(n) => n,
        other => panic!("expected Int, got {other:?}"),
    }
}

fn run_expect_str(src: &str) -> String {
    match run(src).expect("run") {
        VoxValue::Str(s) => s,
        other => panic!("expected Str, got {other:?}"),
    }
}

fn run_expect_bool(src: &str) -> bool {
    match run(src).expect("run") {
        VoxValue::Bool(b) => b,
        other => panic!("expected Bool, got {other:?}"),
    }
}

#[test]
fn parse_ok_returns_result_ok_json() {
    let n = run_expect_int(
        r#"fn main() to int {
            let payload = "{" + "\"x\":7" + "}"
            let r = json.parse(payload)
            if r.is_err() { return -1 }
            let data = r.unwrap()
            return data.get("x").and_then(fn(j: Json) to Option[int] { j.as_int() }).unwrap_or(-1)
        }"#,
    );
    assert_eq!(n, 7);
}

#[test]
fn parse_err_returns_result_err() {
    let s = run_expect_str(
        r#"fn main() to str {
            let r = json.parse("not json at all {{")
            if r.is_err() { return "err" }
            return "ok"
        }"#,
    );
    assert_eq!(s, "err");
}

#[test]
fn get_returns_none_on_missing_key() {
    let s = run_expect_str(
        r#"fn main() to str {
            let payload = "{" + "\"a\":1" + "}"
            let data = json.parse(payload).unwrap()
            return data.get("missing").and_then(fn(j: Json) to Option[str] { j.as_str() }).unwrap_or("(none)")
        }"#,
    );
    assert_eq!(s, "(none)");
}

#[test]
fn at_returns_none_on_oob_and_negative() {
    let s = run_expect_str(
        r#"fn main() to str {
            let payload = "[10,20,30]"
            let data = json.parse(payload).unwrap()
            let oob = data.at(99).and_then(fn(j: Json) to Option[int] { j.as_int() })
            let neg = data.at(-1).and_then(fn(j: Json) to Option[int] { j.as_int() })
            if oob.is_none() and neg.is_none() { return "both-none" }
            return "leaked"
        }"#,
    );
    assert_eq!(s, "both-none");
}

#[test]
fn pointer_walks_deep_path() {
    let n = run_expect_int(
        r#"fn main() to int {
            let inner = "{" + "\"id\":42" + "}"
            let payload = "{" + "\"products\":[" + inner + "]" + "}"
            let data = json.parse(payload).unwrap()
            let leaf = data.pointer("/products/0/id").and_then(fn(j: Json) to Option[int] { j.as_int() })
            return leaf.unwrap_or(-1)
        }"#,
    );
    assert_eq!(n, 42);
}

#[test]
fn pointer_returns_none_on_bad_path() {
    let s = run_expect_str(
        r#"fn main() to str {
            let payload = "{" + "\"a\":1" + "}"
            let data = json.parse(payload).unwrap()
            let r = data.pointer("/nope/0/missing").and_then(fn(j: Json) to Option[str] { j.as_str() })
            if r.is_none() { return "ok" }
            return "leaked"
        }"#,
    );
    assert_eq!(s, "ok");
}

#[test]
fn as_str_returns_none_on_wrong_type() {
    let s = run_expect_str(
        r#"fn main() to str {
            let payload = "{" + "\"n\":42" + "}"
            let data = json.parse(payload).unwrap()
            // n is an int; as_str on an int leaf should yield None.
            return data.get("n").and_then(fn(j: Json) to Option[str] { j.as_str() }).unwrap_or("(none)")
        }"#,
    );
    assert_eq!(s, "(none)");
}

#[test]
fn has_reports_membership_without_unwrapping() {
    let b = run_expect_bool(
        r#"fn main() to bool {
            let payload = "{" + "\"present\":1" + "}"
            let data = json.parse(payload).unwrap()
            return data.has("present") and not data.has("absent")
        }"#,
    );
    assert!(b);
}

#[test]
fn is_null_distinguishes_json_null_value() {
    let s = run_expect_str(
        r#"fn main() to str {
            let payload = "{" + "\"x\":null" + "}"
            let data = json.parse(payload).unwrap()
            let xv = data.get("x").unwrap()
            if xv.is_null() { return "null-value" }
            return "not-null"
        }"#,
    );
    assert_eq!(s, "null-value");
}

#[test]
fn null_value_yields_none_on_leaf_coercion() {
    // RFC §4.3: as_str on a JSON null returns None (not Some("")).
    let s = run_expect_str(
        r#"fn main() to str {
            let payload = "{" + "\"x\":null" + "}"
            let data = json.parse(payload).unwrap()
            return data.get("x").and_then(fn(j: Json) to Option[str] { j.as_str() }).unwrap_or("(none)")
        }"#,
    );
    assert_eq!(s, "(none)");
}
