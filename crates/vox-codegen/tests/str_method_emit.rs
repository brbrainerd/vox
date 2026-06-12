//! Fast emit-to-string tests for Vox string-method lowering.
//!
//! Each test parses a minimal Vox function, emits Rust, and asserts on the
//! generated string — no crate compilation needed (see `emit_compile_harness`
//! for the full type-check layer).
//!
//! Run: `cargo test -p vox-codegen --test str_method_emit -j 4`

use vox_codegen::codegen_rust::emit::emit_fn;
use vox_compiler::hir::lower_module;
use vox_compiler::lexer::lex;
use vox_compiler::parser::parse_script;
use vox_compiler::typeck::typecheck_hir_module;

fn emit_first_fn(src: &str) -> String {
    let module = parse_script(lex(src)).expect("parse");
    let mut hir = lower_module(&module);
    let _ = typecheck_hir_module(src, &mut hir);
    let f = hir.functions.first().expect("at least one function");
    emit_fn(f, Some(&hir.inferred_types), &[])
}

// ── Existing methods (regression guard) ────────────────────────────────────

#[test]
fn slice_two_args() {
    let out = emit_first_fn("fn f(s: str) to str { return s.slice(1, 3) }");
    assert!(
        out.contains("chars().skip"),
        "slice must use chars().skip: {out}"
    );
    assert!(
        out.contains("collect::<String>()"),
        "slice must collect to String: {out}"
    );
}

#[test]
fn char_at_one_arg() {
    let out = emit_first_fn("fn f(s: str) to str { return s.char_at(0) }");
    assert!(
        out.contains("chars().nth"),
        "char_at must use chars().nth: {out}"
    );
    assert!(
        out.contains(".map(|c| c.to_string())"),
        "char_at must map to String: {out}"
    );
}

#[test]
fn index_of_one_arg() {
    let out = emit_first_fn("fn f(s: str, n: str) to int { return s.index_of(n) }");
    assert!(out.contains(".find("), "index_of must use .find: {out}");
    assert!(
        out.contains("chars().count() as i64"),
        "index_of must return char index as i64: {out}"
    );
}

// ── len / is_empty ──────────────────────────────────────────────────────────

#[test]
fn len_casts_to_i64() {
    let out = emit_first_fn("fn f(s: str) to int { return s.len() }");
    assert!(out.contains(".len() as i64"), "len must cast to i64: {out}");
}

#[test]
fn is_empty_bool() {
    let out = emit_first_fn("fn f(s: str) to bool { return s.is_empty() }");
    assert!(
        out.contains(".is_empty()"),
        "is_empty must call .is_empty(): {out}"
    );
}

// ── Case conversion ─────────────────────────────────────────────────────────

#[test]
fn to_upper_maps_to_to_uppercase() {
    let out = emit_first_fn("fn f(s: str) to str { return s.to_upper() }");
    assert!(
        out.contains("to_uppercase()"),
        "to_upper must emit to_uppercase(): {out}"
    );
    assert!(
        !out.contains("to_upper()"),
        "must NOT emit invalid to_upper(): {out}"
    );
}

#[test]
fn to_uppercase_alias() {
    let out = emit_first_fn("fn f(s: str) to str { return s.to_uppercase() }");
    assert!(out.contains("to_uppercase()"), "to_uppercase alias: {out}");
}

#[test]
fn to_lower_maps_to_to_lowercase() {
    let out = emit_first_fn("fn f(s: str) to str { return s.to_lower() }");
    assert!(
        out.contains("to_lowercase()"),
        "to_lower must emit to_lowercase(): {out}"
    );
}

#[test]
fn to_lowercase_alias() {
    let out = emit_first_fn("fn f(s: str) to str { return s.to_lowercase() }");
    assert!(out.contains("to_lowercase()"), "to_lowercase alias: {out}");
}

// ── Trim ────────────────────────────────────────────────────────────────────

#[test]
fn trim_emits_trim_to_string() {
    let out = emit_first_fn("fn f(s: str) to str { return s.trim() }");
    assert!(
        out.contains(".trim().to_string()"),
        "trim must emit .trim().to_string(): {out}"
    );
}

#[test]
fn trim_start_emits_trim_start() {
    let out = emit_first_fn("fn f(s: str) to str { return s.trim_start() }");
    assert!(
        out.contains("trim_start().to_string()"),
        "trim_start: {out}"
    );
}

#[test]
fn trim_end_emits_trim_end() {
    let out = emit_first_fn("fn f(s: str) to str { return s.trim_end() }");
    assert!(out.contains("trim_end().to_string()"), "trim_end: {out}");
}

// ── Pattern methods (arg must be &str) ──────────────────────────────────────

#[test]
fn contains_coerces_arg_to_str() {
    let out = emit_first_fn("fn f(s: str, n: str) to bool { return s.contains(n) }");
    assert!(out.contains("contains("), "contains call missing: {out}");
    // arg must be coerced to &str (as_ref or as_str), NOT passed as String
    assert!(
        out.contains(".as_ref()") || out.contains(".as_str()"),
        "contains arg must be coerced to &str: {out}"
    );
}

#[test]
fn starts_with_coerces_arg() {
    let out = emit_first_fn("fn f(s: str, p: str) to bool { return s.starts_with(p) }");
    assert!(out.contains("starts_with("), "starts_with call: {out}");
    assert!(
        out.contains(".as_ref()") || out.contains(".as_str()"),
        "starts_with arg must be coerced to &str: {out}"
    );
}

#[test]
fn ends_with_coerces_arg() {
    let out = emit_first_fn("fn f(s: str, p: str) to bool { return s.ends_with(p) }");
    assert!(out.contains("ends_with("), "ends_with call: {out}");
    assert!(
        out.contains(".as_ref()") || out.contains(".as_str()"),
        "ends_with arg must be coerced to &str: {out}"
    );
}

// ── split ───────────────────────────────────────────────────────────────────

#[test]
fn split_collects_vec_string() {
    let out = emit_first_fn("fn f(s: str, d: str) to list { return s.split(d) }");
    assert!(
        out.contains("collect::<Vec<String>>()"),
        "split must collect to Vec<String>: {out}"
    );
    assert!(
        out.contains(".as_ref()") || out.contains(".as_str()"),
        "split delim must be coerced to &str: {out}"
    );
}

// ── replace ─────────────────────────────────────────────────────────────────

#[test]
fn replace_two_args() {
    let out = emit_first_fn("fn f(s: str, a: str, b: str) to str { return s.replace(a, b) }");
    assert!(out.contains(".replace("), "replace call: {out}");
    assert!(
        out.contains(".as_ref()") || out.contains(".as_str()"),
        "replace args must be coerced: {out}"
    );
}

// ── repeat ──────────────────────────────────────────────────────────────────

#[test]
fn repeat_emits_usize_cast() {
    let out = emit_first_fn("fn f(s: str, n: int) to str { return s.repeat(n) }");
    assert!(out.contains(".repeat("), "repeat call: {out}");
    assert!(
        out.contains("as usize"),
        "repeat count must cast to usize: {out}"
    );
}

// ── chars_count / count ─────────────────────────────────────────────────────

#[test]
fn chars_count_casts_to_i64() {
    let out = emit_first_fn("fn f(s: str) to int { return s.chars_count() }");
    assert!(
        out.contains("chars().count() as i64"),
        "chars_count must give i64: {out}"
    );
}

#[test]
fn count_with_arg() {
    let out = emit_first_fn("fn f(s: str, sub: str) to int { return s.count(sub) }");
    assert!(out.contains("count"), "count call: {out}");
    assert!(out.contains("i64"), "count must return i64: {out}");
}

// ── Predicate methods ────────────────────────────────────────────────────────

#[test]
fn is_alpha_uses_is_alphabetic() {
    let out = emit_first_fn("fn f(s: str) to bool { return s.is_alpha() }");
    assert!(
        out.contains("is_alphabetic()"),
        "is_alpha must use is_alphabetic: {out}"
    );
}

#[test]
fn is_digit_uses_is_ascii_digit() {
    let out = emit_first_fn("fn f(s: str) to bool { return s.is_digit() }");
    assert!(
        out.contains("is_ascii_digit()"),
        "is_digit must use is_ascii_digit: {out}"
    );
}

#[test]
fn is_alnum_uses_is_alphanumeric() {
    let out = emit_first_fn("fn f(s: str) to bool { return s.is_alnum() }");
    assert!(
        out.contains("is_alphanumeric()"),
        "is_alnum must use is_alphanumeric: {out}"
    );
}

#[test]
fn is_upper_checks_uppercase() {
    let out = emit_first_fn("fn f(s: str) to bool { return s.is_upper() }");
    assert!(
        out.contains("is_uppercase()"),
        "is_upper must check is_uppercase: {out}"
    );
}

#[test]
fn is_lower_checks_lowercase() {
    let out = emit_first_fn("fn f(s: str) to bool { return s.is_lower() }");
    assert!(
        out.contains("is_lowercase()"),
        "is_lower must check is_lowercase: {out}"
    );
}

// ── ord ──────────────────────────────────────────────────────────────────────

#[test]
fn ord_returns_i64() {
    let out = emit_first_fn("fn f(s: str) to int { return s.ord() }");
    assert!(
        out.contains("chars().next()"),
        "ord must use chars().next(): {out}"
    );
    assert!(out.contains("as i64"), "ord must cast to i64: {out}");
}

// ── chars ────────────────────────────────────────────────────────────────────

#[test]
fn chars_collects_vec_string() {
    let out = emit_first_fn("fn f(s: str) to list { return s.chars() }");
    assert!(
        out.contains("collect::<Vec<String>>()"),
        "chars must collect Vec<String>: {out}"
    );
    assert!(
        out.contains("c.to_string()"),
        "chars must map char to String: {out}"
    );
}

// ── to_str / to_string ───────────────────────────────────────────────────────

#[test]
fn to_str_clones() {
    let out = emit_first_fn("fn f(s: str) to str { return s.to_str() }");
    assert!(
        out.contains(".to_string()") || out.contains(".clone()"),
        "to_str must clone/to_string: {out}"
    );
}

// ── to_int / to_float ────────────────────────────────────────────────────────

#[test]
fn to_int_parses_option() {
    let out = emit_first_fn("fn f(s: str) to int { return s.to_int() }");
    assert!(
        out.contains("parse::<i64>()"),
        "to_int must parse as i64: {out}"
    );
    assert!(
        out.contains(".ok()"),
        "to_int must use .ok() for Option: {out}"
    );
}

#[test]
fn to_float_parses_option() {
    let out = emit_first_fn("fn f(s: str) to float { return s.to_float() }");
    assert!(
        out.contains("parse::<f64>()"),
        "to_float must parse as f64: {out}"
    );
    assert!(
        out.contains(".ok()"),
        "to_float must use .ok() for Option: {out}"
    );
}
