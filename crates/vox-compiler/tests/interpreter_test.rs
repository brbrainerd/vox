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
        vox_compiler::eval::value::VoxValue::List(vec![
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
            assert!(ms > 0, "std.time.now_ms() should be a positive epoch ms, got {ms}");
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
            assert!(
                !msg.contains("--mode script"),
                "std.http should perform a real request in interp, not return a stub: {msg}"
            );
        }
        other => panic!("std.http.get_text on an invalid URL should return Result::Err, got {other:?}"),
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
