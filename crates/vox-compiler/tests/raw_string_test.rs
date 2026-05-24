//! Regression test for raw-string literal syntax `r"..."` (2026-05-23).
//!
//! Raw strings preserve backslashes verbatim and do NOT process escape
//! sequences. Used for regex patterns, Windows paths, and any string
//! where `\n`/`\t`/`\"` shouldn't be interpreted.

use vox_compiler::lexer::lex;
use vox_compiler::lexer::token::Token;

fn first_token(src: &str) -> Token {
    lex(src)
        .into_iter()
        .map(|s| s.token)
        .find(|t| !matches!(t, Token::Eof))
        .expect("at least one token")
}

#[test]
fn raw_string_preserves_backslashes() {
    let src = r##"r"a\nb""##;
    match first_token(src) {
        Token::RawStringLit(s) => assert_eq!(s, r"a\nb"),
        other => panic!("expected RawStringLit, got {other:?}"),
    }
}

#[test]
fn raw_string_holds_regex_with_capture_group() {
    let src = r##"r"CREATE\s+TABLE\s+([a-z_]+)""##;
    match first_token(src) {
        Token::RawStringLit(s) => {
            assert_eq!(s, r"CREATE\s+TABLE\s+([a-z_]+)");
        }
        other => panic!("expected RawStringLit, got {other:?}"),
    }
}

#[test]
fn raw_string_holds_windows_path() {
    let src = r##"r"C:\Users\Owner\vox""##;
    match first_token(src) {
        Token::RawStringLit(s) => assert_eq!(s, r"C:\Users\Owner\vox"),
        other => panic!("expected RawStringLit, got {other:?}"),
    }
}

#[test]
fn raw_string_with_brace_does_not_become_template() {
    // The regex inside is `{0,3}` — would be problematic in a regular
    // string due to the `{` template trigger. Raw form sidesteps that.
    let src = r##"r"a{0,3}b""##;
    assert!(matches!(first_token(src), Token::RawStringLit(_)));
}

#[test]
fn regular_string_still_tokenizes_after_raw_string_lands() {
    let src = r#""regular""#;
    match first_token(src) {
        Token::StringLit(s) => assert_eq!(s, "regular"),
        other => panic!("expected StringLit, got {other:?}"),
    }
}

#[test]
fn ident_starting_with_r_is_not_swallowed() {
    // `rate` should be an Ident, not the start of a raw-string. Critical
    // because `r"..."` only fires when `r` is *immediately* followed by `"`.
    let src = "rate";
    match first_token(src) {
        Token::Ident(s) => assert_eq!(s, "rate"),
        other => panic!("expected Ident(\"rate\"), got {other:?}"),
    }
}
