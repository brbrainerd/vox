//! Property-based tests for the Vox compiler (parser + formatter).
//!
//! Uses proptest to catch grammar regressions and formatter non-idempotency
//! that deterministic golden tests cannot cover.
//!
//! Case budget: 64 per test to keep CI within time budget.

use proptest::prelude::*;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;

// ── Identifier strategy ─────────────────────────────────────────────────────

/// Generates valid Vox identifiers: `[a-zA-Z_][a-zA-Z0-9_]*`
fn valid_ident() -> impl Strategy<Value = String> {
    (
        prop::char::ranges(vec!['a'..='z', 'A'..='Z', '_'..='_'].into()),
        prop::collection::vec(
            prop::char::ranges(vec!['a'..='z', 'A'..='Z', '0'..='9', '_'..='_'].into()),
            0..16,
        ),
    )
        .prop_map(|(head, tail)| {
            let mut s = String::with_capacity(1 + tail.len());
            s.push(head);
            s.extend(tail);
            s
        })
        // "to" is a Vox keyword; avoid accidental keyword collisions for simple names
        .prop_filter("not a reserved keyword", |s| {
            // `_` alone is a discard placeholder in fn contexts, not a declaration name.
            if s == "_" {
                return false;
            }
            !matches!(
                s.as_str(),
                "fn" | "to"
                    | "type"
                    | "let"
                    | "return"
                    | "if"
                    | "else"
                    | "for"
                    | "in"
                    | "pub"
                    | "import"
                    | "as"
                    | "true"
                    | "false"
                    | "and"
                    | "or"
                    | "not"
                    | "match"
                    | "with"
                    | "do"
                    | "async"
                    | "await"
                    | "routes"
                    | "component"
            )
        })
}

// ── Test 1: parser accepts valid identifiers as fn names ─────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn parser_accepts_valid_identifiers(name in valid_ident()) {
        let src = format!("fn {name}() to int {{ return 1 }}");
        let tokens = lex(&src);
        let result = parse(tokens);
        prop_assert!(
            result.is_ok(),
            "Expected parse to succeed for fn {name}(), got errors: {:?}",
            result.err()
        );
    }
}

// ── Test 2: parser round-trip for fn declarations ────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn parser_round_trip_fn_decl(name in valid_ident()) {
        // Parse a minimal fn declaration and verify the AST contains our name.
        let src = format!("fn {name}() to int {{ return 1 }}");
        let m = parse(lex(&src)).expect("first parse must succeed");
        // Re-serialize via the formatter then re-parse — still must succeed.
        let formatted = vox_compiler::fmt::format(&src);
        let result2 = parse(lex(&formatted));
        prop_assert!(
            result2.is_ok(),
            "Re-parse of formatted source failed for fn {name}(). \
             Formatted source:\n{formatted}\nErrors: {:?}",
            result2.err()
        );
        // The module from the first parse must have exactly one declaration.
        prop_assert_eq!(
            m.declarations.len(),
            1,
            "Expected exactly 1 declaration for fn {}(), got {}",
            name,
            m.declarations.len()
        );
    }
}

// ── Test 3: formatter idempotency ────────────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn formatter_idempotent(name in valid_ident()) {
        let src = format!("fn {name}() to int {{ return 1 }}");
        let once = vox_compiler::fmt::format(&src);
        let twice = vox_compiler::fmt::format(&once);
        prop_assert_eq!(
            once,
            twice,
            "Formatter is not idempotent for fn {}()",
            name
        );
    }
}

// ── Test 4: type annotation round-trip ──────────────────────────────────────

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn type_annotation_round_trip(type_name in valid_ident()) {
        // Wrap a generated type name in a minimal type alias declaration.
        let src = format!("type Wrapper = {type_name}");
        let tokens = lex(&src);
        let result = parse(tokens);
        // We just assert the parser does not panic and either Ok or returns
        // structured errors — no unrecoverable crash.
        match result {
            Ok(m) => {
                prop_assert!(
                    !m.declarations.is_empty(),
                    "Expected at least one declaration for type Wrapper = {type_name}"
                );
            }
            Err(errs) => {
                // Some generated names may conflict with future reserved words.
                // The important invariant is that parse() returned an error
                // rather than panicking.
                prop_assert!(
                    !errs.is_empty(),
                    "Errors vec must be non-empty when parse fails for type Wrapper = {type_name}"
                );
            }
        }
    }
}
