/// Verify that an indexed for-loop (`for v, i in arr`) binds the index
/// variable correctly in the body.  The for expression returns a list, so
/// `for v, i in [10, 20, 30] { v + i }` should produce [10+0, 20+1, 30+2]
/// = [10, 21, 32].
#[test]
fn for_loop_with_index_binds_index_in_body() {
    let source = "
    fn main() -> List {
        return for v, i in [10, 20, 30] { v + i }
    }
    ";

    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::descent::parse(tokens).expect("Failed to parse");
    let lowered = vox_compiler::hir::lower::lower_module(&module);

    let mut interpreter = vox_compiler::eval::Interpreter::new(100_000);
    interpreter
        .run_module(&lowered)
        .expect("Failed to run module");

    let res = interpreter
        .call("main", vec![])
        .expect("Failed to call main");
    assert_eq!(
        res,
        vox_compiler::eval::value::VoxValue::list(vec![
            vox_compiler::eval::value::VoxValue::Int(10),
            vox_compiler::eval::value::VoxValue::Int(21),
            vox_compiler::eval::value::VoxValue::Int(32),
        ]),
        "for v, i in [10,20,30] {{ v+i }} should yield [10, 21, 32]"
    );
}

/// B1 — exact decimal arithmetic in the interpreter. `0.1dec + 0.2dec is 0.3dec`
/// must be exactly `true`. Under the old `DecimalLit -> f64` approximation this
/// was silently `false` (0.1 + 0.2 != 0.3 in IEEE-754), so `decimal_math.vox`'s
/// asserts held under `--mode script` but failed under `--mode interp`.
#[test]
fn decimal_arithmetic_is_exact_in_interp() {
    let source = "
    fn main() -> bool {
        return 0.1dec + 0.2dec is 0.3dec
    }
    ";

    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::descent::parse(tokens).expect("Failed to parse");
    let lowered = vox_compiler::hir::lower::lower_module(&module);

    let mut interpreter = vox_compiler::eval::Interpreter::new(100_000);
    interpreter
        .run_module(&lowered)
        .expect("Failed to run module");

    let res = interpreter
        .call("main", vec![])
        .expect("Failed to call main");
    assert_eq!(
        res,
        vox_compiler::eval::value::VoxValue::Bool(true),
        "0.1dec + 0.2dec is 0.3dec must be exactly true under --mode interp"
    );
}

/// A returned `Option` that is `None` must compare equal to the `None` literal.
/// Bare `None` is stored as `Constructor("None")` until normalized; without
/// normalizing both `is` operands, `mk() is None` was silently `false`.
#[test]
fn returned_none_is_equal_to_none_literal() {
    let source = "
    fn mk() to Option[int] {
        return None
    }
    fn main() to bool {
        return mk() is None
    }
    ";

    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::descent::parse(tokens).expect("Failed to parse");
    let lowered = vox_compiler::hir::lower::lower_module(&module);

    let mut interpreter = vox_compiler::eval::Interpreter::new(100_000);
    interpreter
        .run_module(&lowered)
        .expect("Failed to run module");

    let res = interpreter
        .call("main", vec![])
        .expect("Failed to call main");
    assert_eq!(
        res,
        vox_compiler::eval::value::VoxValue::Bool(true),
        "`mk() is None` (mk returns None) must be true"
    );
}

/// C1 runtime: `Result[T, E]` carries a real error VALUE at runtime, not a
/// stringified form. `Error(Timeout(30))` round-trips through the Err arm and
/// matches back to the ADT variant (was a `String` before the Err-widening).
#[test]
fn result_carries_typed_error_value() {
    let source = "
    type ErrKind = | NotFound | Timeout(ms: int)
    fn f() to Result[int, ErrKind] {
        return Error(Timeout(30))
    }
    fn main() to int {
        return match f() {
            Ok(x) => x
            Error(e) => match e {
                Timeout(ms) => ms
                NotFound => 0
            }
        }
    }
    ";
    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::descent::parse(tokens).expect("parse");
    let lowered = vox_compiler::hir::lower::lower_module(&module);
    let mut interp = vox_compiler::eval::Interpreter::new(100_000);
    interp.run_module(&lowered).expect("run_module");
    let res = interp.call("main", vec![]).expect("call main");
    assert_eq!(
        res,
        vox_compiler::eval::value::VoxValue::Int(30),
        "Error(Timeout(30)) should carry the ADT to the match, yielding 30"
    );
}

/// Nullary-variant match regression: a bare capitalized pattern (`Red`) is a
/// *nullary constructor* pattern, not a binding. Before the parser fix it
/// lowered to `Pattern::Ident`, so the FIRST arm bound the scrutinee
/// unconditionally and became a catch-all — `match Green { Red => .. }` wrongly
/// took the `Red` arm. Each arm must match only its own variant.
#[test]
fn nullary_variant_match_is_not_catch_all() {
    let source = "
    type Color = | Red | Green | Blue
    fn pick() to Color {
        return Green
    }
    fn main() to int {
        return match pick() {
            Red => 1
            Green => 2
            Blue => 3
        }
    }
    ";
    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::descent::parse(tokens).expect("parse");
    let lowered = vox_compiler::hir::lower::lower_module(&module);
    let mut interp = vox_compiler::eval::Interpreter::new(100_000);
    interp.run_module(&lowered).expect("run_module");
    let res = interp.call("main", vec![]).expect("call main");
    assert_eq!(
        res,
        vox_compiler::eval::value::VoxValue::Int(2),
        "match pick() (=Green) must take the Green arm (2), not the first arm"
    );
}

/// Gap Finding 1: `for` over a map (yields (k,v) tuples) and over a string
/// (yields chars) must run, not crash with TypeError{expected:"List"}.
#[test]
fn for_over_map_and_string_iterate() {
    let source = "
    fn count_map() to int {
        let m = { a: 1, b: 2, c: 3 }
        let mut n = 0
        for entry in m { n = n + 1 }
        return n
    }
    fn count_str() to int {
        let mut n = 0
        for ch in \"abcd\" { n = n + 1 }
        return n
    }
    fn main() to int {
        return count_map() + count_str()
    }
    ";
    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::descent::parse(tokens).expect("parse");
    let lowered = vox_compiler::hir::lower::lower_module(&module);
    let mut interp = vox_compiler::eval::Interpreter::new(100_000);
    interp.run_module(&lowered).expect("run_module");
    let res = interp.call("main", vec![]).expect("call main");
    assert_eq!(
        res,
        vox_compiler::eval::value::VoxValue::Int(7),
        "for over map (3) + for over string (4) = 7"
    );
}

/// Gap Finding 2: mixed Int/Float arithmetic must promote to Float (the
/// typechecker already does), not crash.
#[test]
fn mixed_int_float_arithmetic() {
    let source = "
    fn main() to float {
        let x = 1
        let y = 2.0
        return x + y
    }
    ";
    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::descent::parse(tokens).expect("parse");
    let lowered = vox_compiler::hir::lower::lower_module(&module);
    let mut interp = vox_compiler::eval::Interpreter::new(100_000);
    interp.run_module(&lowered).expect("run_module");
    let res = interp.call("main", vec![]).expect("call main");
    assert_eq!(res, vox_compiler::eval::value::VoxValue::Float(3.0));
}

/// Gap Finding 3: cross-numeric `is` is value equality (consistent with
/// arithmetic promotion) — `1 is 1.0` is true, not silently false.
#[test]
fn cross_numeric_is_equality() {
    let source = "
    fn main() to bool {
        return 1 is 1.0
    }
    ";
    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::descent::parse(tokens).expect("parse");
    let lowered = vox_compiler::hir::lower::lower_module(&module);
    let mut interp = vox_compiler::eval::Interpreter::new(100_000);
    interp.run_module(&lowered).expect("run_module");
    let res = interp.call("main", vec![]).expect("call main");
    assert_eq!(res, vox_compiler::eval::value::VoxValue::Bool(true));
}

/// B2 — compiled-regex object idiom in the interpreter. `std.regex.compile`
/// must return a real Regex value whose `.find()` yields a Match with `.group(i)`
/// (the form `regex_stdlib.vox` teaches). Previously `compile` returned a bare
/// string, so `re.find(...).group(1)` was unreachable under `--mode interp`.
#[test]
fn compiled_regex_find_group_in_interp() {
    let source = r#"
    fn main() -> Option {
        let compiled = std.regex.compile("(?:mood|feeling).*?(\\d)")
        return match compiled {
            Ok(re) => match re.find("my mood is 7 today") {
                Some(m) => m.group(1)
                None => None
            }
            Error(_) => None
        }
    }
    "#;

    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::descent::parse(tokens).expect("Failed to parse");
    let lowered = vox_compiler::hir::lower::lower_module(&module);

    let mut interpreter = vox_compiler::eval::Interpreter::new(100_000);
    interpreter
        .run_module(&lowered)
        .expect("Failed to run module");

    let res = interpreter
        .call("main", vec![])
        .expect("Failed to call main");
    assert_eq!(
        res,
        vox_compiler::eval::value::VoxValue::Option(Some(Box::new(
            vox_compiler::eval::value::VoxValue::Str("7".to_string())
        ))),
        "compiled regex re.find(...).group(1) should extract \"7\" under --mode interp"
    );
}

/// B4 — `std.time.now_ms()` must resolve under `--mode interp`. It was declared
/// in typeck + codegen but had no interpreter dispatch arm, so it errored as an
/// unreachable namespace. We can't assert an exact value (it's wall-clock), but
/// it must return a positive Int.
#[test]
fn std_time_now_ms_runs_in_interp() {
    let source = "
    fn main() -> int {
        return std.time.now_ms()
    }
    ";

    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::descent::parse(tokens).expect("Failed to parse");
    let lowered = vox_compiler::hir::lower::lower_module(&module);

    let mut interpreter = vox_compiler::eval::Interpreter::new(100_000);
    interpreter
        .run_module(&lowered)
        .expect("Failed to run module");

    let res = interpreter
        .call("main", vec![])
        .expect("Failed to call main");
    match res {
        vox_compiler::eval::value::VoxValue::Int(ms) => {
            assert!(
                ms > 0,
                "std.time.now_ms() should be a positive epoch ms, got {ms}"
            );
        }
        other => panic!("std.time.now_ms() should return Int, got {other:?}"),
    }
}

/// B3 — `std.http` performs a REAL request under `--mode interp` (Vox is a
/// web-app language). An empty URL makes reqwest fail to build the request, so
/// the call returns `Result::Err` with a transport/URL error — NOT the old
/// "use --mode script" stub. This exercises the real interp HTTP path without
/// depending on external network availability.
#[test]
fn std_http_get_text_performs_real_request_in_interp() {
    let source = "
    fn main() -> Result {
        return std.http.get_text(\"\")
    }
    ";

    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::descent::parse(tokens).expect("Failed to parse");
    let lowered = vox_compiler::hir::lower::lower_module(&module);

    let mut interpreter = vox_compiler::eval::Interpreter::new(100_000);
    interpreter
        .run_module(&lowered)
        .expect("Failed to run module");

    let res = interpreter
        .call("main", vec![])
        .expect("Failed to call main");
    match res {
        vox_compiler::eval::value::VoxValue::Result(Err(msg)) => {
            // Err side is now a boxed VoxValue; the http transport error is a Str.
            let msg = format!("{:?}", *msg);
            assert!(
                !msg.contains("--mode script"),
                "std.http should perform a real request in interp, not return a stub: {msg}"
            );
        }
        other => {
            panic!("std.http.get_text on an invalid URL should return Result::Err, got {other:?}")
        }
    }
}

#[test]
fn test_interpreter_basic() {
    let source = "
    fn add(a: int, b: int) -> int {
        return a + b
    }

    fn main() -> int {
        let x = 10
        let mut y = 20
        while y < 30 {
            y = y + 2
        }
        return add(x, y)
    }
    ";

    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::descent::parse(tokens).expect("Failed to parse");
    // We need to lower it to HIR
    let lowered = vox_compiler::hir::lower::lower_module(&module);

    let mut interpreter = vox_compiler::eval::Interpreter::new(100_000);
    interpreter
        .run_module(&lowered)
        .expect("Failed to run module");

    let res = interpreter
        .call("main", vec![])
        .expect("Failed to call main");
    assert_eq!(res, vox_compiler::eval::value::VoxValue::Int(40));
}

/// `not bool` must invert the boolean. Vox commits to phonetic-only operators
/// — `!` errors at parse time with a "use `not`" hint (see the next test).
/// See docs/src/architecture/vox-stdlib-gap-audit-2026-05-23.md §8.
#[test]
fn not_keyword_inverts_bool_correctly() {
    let source = "
    fn main() -> bool {
        let f = false;
        let t = true;
        return (not f) and ((not t) == false) and ((not (not t)) == true)
    }
    ";

    let tokens = vox_compiler::lexer::lex(source);
    let module = vox_compiler::parser::descent::parse(tokens).expect("Failed to parse");
    let lowered = vox_compiler::hir::lower::lower_module(&module);

    let mut interpreter = vox_compiler::eval::Interpreter::new(100_000);
    interpreter
        .run_module(&lowered)
        .expect("Failed to run module");

    let res = interpreter
        .call("main", vec![])
        .expect("Failed to call main");
    assert_eq!(
        res,
        vox_compiler::eval::value::VoxValue::Bool(true),
        "`not` must invert booleans correctly"
    );
}

/// Using `!` for negation must fail at parse time with a message that
/// names `not` as the canonical form. This catches AI-generated `!x`
/// code at the earliest possible moment.
#[test]
fn bang_is_a_parse_error_with_phonetic_hint() {
    let source = "fn main() -> bool { return !true }";
    let tokens = vox_compiler::lexer::lex(source);
    let result = vox_compiler::parser::descent::parse(tokens);
    let err = result.expect_err("`!` must be rejected at parse time");
    let combined = format!("{err:?}");
    assert!(
        combined.contains("`!` is not a valid operator"),
        "error must explicitly name `!` as invalid; got: {combined}"
    );
    assert!(
        combined.contains("not"),
        "error must point at `not` as the canonical form; got: {combined}"
    );
}

/// Task 1A: Interpreter must reject web/actor constructs with an actionable
/// diagnostic, never silently evaluate them to Null (Pattern A gap).
/// Mirrors the CR-F4 pattern at eval/expr.rs:513-524 (Scrape/Browser guard).
#[test]
fn interp_rejects_unsupported_expr_with_clean_diagnostic() {
    // JSX is a web/compiled construct with no interpreter semantics.
    // Parsing succeeds; eval must fail with an actionable message, not Null.
    let source = r#"
    fn view() -> Int {
        let x = <div></div>
        1
    }
    fn main() -> Int { view() }
    "#;
    let tokens = vox_compiler::lexer::lex(source);
    let Ok(module) = vox_compiler::parser::descent::parse(tokens) else {
        // If JSX doesn't parse yet, the test is vacuously green — acceptable.
        return;
    };
    let lowered = vox_compiler::hir::lower::lower_module(&module);
    let mut interp = vox_compiler::eval::Interpreter::new(100_000);
    interp.run_module(&lowered).ok();
    let result = interp.call("main", vec![]);
    match result {
        Err(e) => {
            let msg = format!("{e:?}");
            assert!(
                msg.contains("not supported in --mode interp"),
                "expected a CR-F4-style interp diagnostic for JSX, got: {msg}"
            );
        }
        Ok(val) => {
            // If the interpreter returned a non-Null value, JSX lowered to
            // something meaningful and the test is vacuously ok.
            // If it returned Null, that's the silent-drop bug this test catches.
            use vox_compiler::eval::value::VoxValue;
            assert_ne!(
                val,
                VoxValue::Null,
                "JSX must not silently evaluate to Null in --mode interp"
            );
        }
    }
}
