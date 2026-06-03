//! C2 — exhaustiveness for the built-in sum types Option and Result.
//!
//! Previously only `Bool` and named ADTs were exhaustiveness-checked; matches on
//! `Option`/`Result` (the two most-matched types) fell through unchecked, so a
//! match missing `None` or `Error` compiled clean. These tests pin the new
//! `E0301` behavior.

use vox_compiler::lexer::lex;
use vox_compiler::parser::parse;
use vox_compiler::typeck::diagnostics::TypeckSeverity;
use vox_compiler::typeck::typecheck_module;

fn error_codes(src: &str) -> Vec<String> {
    let module = parse(lex(src)).expect("source should parse");
    typecheck_module(&module, src)
        .into_iter()
        .filter(|d| d.severity == TypeckSeverity::Error)
        .map(|d| format!("{}: {}", d.code.as_deref().unwrap_or(""), d.message))
        .collect()
}

#[test]
fn non_exhaustive_option_match_is_rejected() {
    let src = "
    fn f(o: Option[int]) to int {
        return match o {
            Some(x) => x
        }
    }
    ";
    let errs = error_codes(src);
    assert!(
        errs.iter().any(|e| e.contains("E0301")),
        "match on Option missing `None` should be E0301; got {errs:?}"
    );
}

#[test]
fn non_exhaustive_result_match_is_rejected() {
    let src = "
    fn f(r: Result[int]) to int {
        return match r {
            Ok(x) => x
        }
    }
    ";
    let errs = error_codes(src);
    assert!(
        errs.iter().any(|e| e.contains("E0301")),
        "match on Result missing the error arm should be E0301; got {errs:?}"
    );
}

#[test]
fn exhaustive_option_and_result_matches_are_accepted() {
    let src = "
    fn f(o: Option[int]) to int {
        return match o {
            Some(x) => x
            None => 0
        }
    }
    fn g(r: Result[int]) to int {
        return match r {
            Ok(x) => x
            Error(e) => 0
        }
    }
    fn h(o: Option[int]) to int {
        return match o {
            Some(x) => x
            other => 0
        }
    }
    ";
    let errs = error_codes(src);
    assert!(
        !errs.iter().any(|e| e.contains("E0301")),
        "exhaustive Option/Result matches (incl. binding-as-wildcard) should not error; got {errs:?}"
    );
}
