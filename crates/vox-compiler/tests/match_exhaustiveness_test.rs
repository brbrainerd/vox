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
fn db_query_chaining_typechecks() {
    // B5: chained db query plan methods (.where/.order_by/.limit) typecheck to
    // Result[List[Record]] instead of erroring ("method not found" / "not
    // supported yet"). The Rust codegen already emits the SQL.
    let src = "
    @table type Task {
        title: str
        done: bool
    }
    @query fn active() to int {
        let rows = db.Task.where({ done: { eq: false } }).order_by(\"title\").limit(10)
        return len(rows)
    }
    ";
    let errs = error_codes(src);
    assert!(
        errs.is_empty(),
        "chained db query (.where/.order_by/.limit) should typecheck; got {errs:?}"
    );
}

#[test]
fn result_error_arm_binds_declared_error_type() {
    // C1: `Result[T, E]` threads the error type. The `Error(code)` arm must bind
    // `code` to the declared `E` (here `int`) — NOT the historical hardcoded
    // `str` — so `code + 1` typechecks. Under the old single-param `Result`,
    // `code` was `str` and `code + 1` would be a type error.
    let src = "
    fn f(r: Result[str, int]) to int {
        return match r {
            Ok(s) => len(s)
            Error(code) => code + 1
        }
    }
    ";
    let errs = error_codes(src);
    assert!(
        errs.is_empty(),
        "Result[str, int] should bind the Error arm payload as int (the E); got {errs:?}"
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

/// A typed error ADT — `Error(NotFound)` for `Result[int, MyErr]` — type-checks.
/// Before `Error` was made polymorphic in its error type it forced a `str`
/// payload, so `Error(NotFound)` failed with "Cannot unify Str with Named(...)".
#[test]
fn typed_error_adt_construction_typechecks() {
    let src = "
    type Err = | NotFound | Timeout
    fn f() to Result[int, Err] {
        return Error(NotFound)
    }
    fn main() to int {
        return match f() {
            Ok(x) => x
            Error(e) => match e {
                NotFound => 0
                Timeout => 1
            }
        }
    }
    ";
    let errs = error_codes(src);
    assert!(
        errs.is_empty(),
        "typed-error construction + exhaustive match should type-check; got {errs:?}"
    );
}

/// Matching the error ADT inside the `Error(e)` arm must be exhaustive — a
/// missing variant is rejected just like any other ADT match.
#[test]
fn non_exhaustive_error_adt_match_is_rejected() {
    let src = "
    type Err = | NotFound | Timeout
    fn f() to Result[int, Err] {
        return Error(NotFound)
    }
    fn main() to int {
        return match f() {
            Ok(x) => x
            Error(e) => match e {
                NotFound => 0
            }
        }
    }
    ";
    let errs = error_codes(src);
    assert!(
        errs.iter()
            .any(|e| e.contains("Missing variant(s): Timeout")),
        "non-exhaustive error-ADT match should be rejected; got {errs:?}"
    );
}

/// Backward compatibility: a bare string error still infers `E = str`.
#[test]
fn string_error_still_typechecks() {
    let src = "
    fn f() to Result[int, str] {
        return Error(\"boom\")
    }
    fn main() to int {
        return match f() {
            Ok(x) => x
            Error(e) => 0
        }
    }
    ";
    let errs = error_codes(src);
    assert!(
        errs.is_empty(),
        "string-error construction should still type-check; got {errs:?}"
    );
}

/// `Ok(v)` constructed in a function declared `to Result[T, MyErr]` must unify
/// its error parameter with the declared ADT, not force `E = str`. Before `Ok`
/// was made polymorphic in `E` this failed with "Cannot unify Str with
/// Named(...)" (the marquee slot-1 regression).
#[test]
fn ok_construction_in_typed_result_typechecks() {
    let src = "
    type AddItemError = | Duplicate | Invalid
    fn add(name: str) to Result[str, AddItemError] {
        if name is \"\" {
            return Error(Invalid)
        }
        return Ok(\"added\")
    }
    fn main() to str {
        return match add(\"x\") {
            Ok(s) => s
            Error(e) => match e {
                Duplicate => \"dup\"
                Invalid => \"bad\"
            }
        }
    }
    ";
    let errs = error_codes(src);
    assert!(
        errs.is_empty(),
        "Ok/Error in a typed Result[T, MyErr] fn should type-check; got {errs:?}"
    );
}
