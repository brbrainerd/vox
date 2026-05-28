//! Regression test for the lexer disambiguation between JSON-literal strings
//! and template strings.
//!
//! The `TemplateStringLit` regex must require `{` to be followed by an
//! identifier-start character (after optional whitespace). Without this guard,
//! every JSON literal containing `{"key":...}` was misclassified as a
//! template, producing "Complex expressions in template strings not yet
//! supported" parse errors and breaking ~3 corpus scripts.
//!
//! See: `docs/src/architecture/json-ergonomics-rfc-2026-05-23.md` §10
//! migration impact; lexer change at
//! `crates/vox-compiler/src/lexer/token.rs:390`.

use vox_compiler::lexer::lex;
use vox_compiler::lexer::token::Token;

/// Returns the first non-Eof token's class name and payload (if any),
/// stripped to make assertions readable.
fn first_token(src: &str) -> Token {
    let tokens = lex(src);
    tokens
        .into_iter()
        .map(|s| s.token)
        .find(|t| !matches!(t, Token::Eof))
        .expect("at least one token")
}

#[test]
fn json_object_literal_is_string_lit_not_template() {
    let src = r#""{\"key\":1}""#;
    match first_token(src) {
        Token::StringLit(s) => {
            // Lexer unescapes `\"` → `"` in the string value.
            assert_eq!(s, r#"{"key":1}"#);
        }
        other => panic!("expected StringLit, got {other:?}"),
    }
}

#[test]
fn json_object_literal_with_array_and_nested_is_string_lit() {
    let src = r#""{\"items\":[1,2,3],\"meta\":{\"v\":1}}""#;
    assert!(matches!(first_token(src), Token::StringLit(_)));
}

#[test]
fn json_empty_object_literal_is_string_lit() {
    let src = r#""{}""#;
    assert!(matches!(first_token(src), Token::StringLit(_)));
}

#[test]
fn template_string_with_ident_is_template_lit() {
    let src = r#""hello, {name}!""#;
    match first_token(src) {
        Token::TemplateStringLit(s) => {
            assert_eq!(s, "hello, {name}!");
        }
        other => panic!("expected TemplateStringLit, got {other:?}"),
    }
}

#[test]
fn template_string_with_leading_whitespace_in_braces_is_template_lit() {
    // The lexer fix allows optional whitespace before the identifier inside
    // the braces so `{ name }` (legal Vox) still parses as a template.
    let src = r#""hi { name }""#;
    assert!(matches!(first_token(src), Token::TemplateStringLit(_)));
}

#[test]
fn plain_string_without_braces_is_string_lit() {
    let src = r#""just a string""#;
    match first_token(src) {
        Token::StringLit(s) => assert_eq!(s, "just a string"),
        other => panic!("expected StringLit, got {other:?}"),
    }
}

#[test]
fn json_with_underscore_key_is_string_lit() {
    // Edge case: `{_internal:...}` — `_internal` IS an identifier-start
    // sequence, so a raw `{_internal:1}` would *look* like a template.
    // But JSON requires `{"_internal":1}` (quoted key), so the leading `\"`
    // still saves us. Document the boundary.
    let src = r#""{\"_internal\":1}""#;
    assert!(matches!(first_token(src), Token::StringLit(_)));
}
