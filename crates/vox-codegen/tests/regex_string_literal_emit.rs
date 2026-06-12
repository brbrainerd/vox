//! Fast emit-to-string tests for regex pattern string literals.
//!
//! Vox source strings like `"\\w+"` hold regex escapes (`\w`). codegen-rust must
//! emit valid Rust (`r"..."` or `"\\w+"`), not `"\\w+"` with unknown `\w` escapes.
//!
//! Run: `cargo test -p vox-codegen --test regex_string_literal_emit --quiet`

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

/// Emitted Rust is valid when regex backslash sequences are raw or doubled.
fn emitted_regex_escape_is_valid_rust(out: &str) -> bool {
    out.contains(r#"r"\w"#)
        || out.contains(r#"r#"\w"#)
        || out.contains(r#"\\w"#)
        || out.contains(r#"\\d"#)
        || out.contains(r#"\\."#)
        || out.contains(r#"\\b"#)
}

#[test]
fn regex_is_match_pattern_string_literal_escapes_backslashes() {
    let out = emit_first_fn(
        r#"fn f() to bool { return regex.is_match("user@example.com", "^[\\w.+-]+@[\\w-]+\\.[a-z]{2,}$") }"#,
    );
    assert!(
        emitted_regex_escape_is_valid_rust(&out),
        "regex pattern must emit as raw or escaped Rust string, got:\n{out}"
    );
    assert!(
        !out.contains(r#""\w"#) || out.contains(r#""\\w"#) || out.contains(r#"r"\w"#),
        "must not emit unknown Rust escape \\w in a normal string literal, got:\n{out}"
    );
}

#[test]
fn bare_regex_pattern_string_literal_escapes_backslashes() {
    let out = emit_first_fn(r#"fn f() to str { return "\\w+" }"#);
    assert!(
        emitted_regex_escape_is_valid_rust(&out),
        "bare regex-style string must emit valid Rust, got:\n{out}"
    );
}
