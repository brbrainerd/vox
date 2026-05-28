//! Diagnostic repro: which EvalError do the Result-pattern test fixtures
//! actually hit? Used to drive Task K (interpreter Option/Result match
//! support). This file stays as a regression guard once the fix lands.

#[test]
fn parse_int_test_executes_via_in_process_interpreter() {
    use vox_compiler::eval::Interpreter;

    let source = r#"
fn parse_int(s: str) to Result[int] {
    if len(s) is 0 {
        return Error("empty string")
    }
    return Ok(42)
}

@test
fn test_empty_is_error() to Unit {
    let r = parse_int("")
    match r {
        Error(_) => assert(true)
        Ok(_)    => assert(false)
    }
}

@test
fn test_nonempty_is_ok() to Unit {
    let r = parse_int("42")
    match r {
        Ok(n)    => assert(n is 42)
        Error(_) => assert(false)
    }
}
"#;
    let res = vox_compiler::pipeline::run_frontend_str(source, "parse_repro.vox")
        .expect("frontend should succeed");
    let mut interp = Interpreter::new(10_000);
    interp
        .run_module(&res.hir)
        .expect("run_module should succeed");
    for t in &res.hir.tests {
        let outcome = interp.call(&t.name, Vec::new());
        assert!(
            outcome.is_ok(),
            "test `{}` should run to completion without eval error; got {:?}",
            t.name,
            outcome.err()
        );
    }
}
