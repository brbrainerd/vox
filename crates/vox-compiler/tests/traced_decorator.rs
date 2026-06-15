//! TRACE-D P7: `@traced` decorator — parse, lower, and interpreter acceptance tests.

use vox_compiler::eval::Interpreter;
use vox_compiler::eval::value::VoxValue;
use vox_compiler::hir::lower::lower_module;
use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse_script;

fn interp_for(src: &str) -> Interpreter {
    let module = lower_module(&parse_script(lex(src)).expect("parse_script"));
    let mut interp = Interpreter::new(1_000_000);
    interp.run_module(&module).expect("run_module");
    interp
}

/// `VoxValue::Fn` registered in the interpreter scope must carry `is_traced = true`.
#[test]
fn traced_fn_value_carries_flag() {
    let interp = interp_for("@traced fn process() to int { return 1 }");
    match interp.scope.get("process") {
        Some(VoxValue::Fn {
            is_traced, name, ..
        }) => {
            assert!(*is_traced, "registered fn must carry is_traced");
            assert_eq!(name, "process");
        }
        other => panic!("expected Fn, got {other:?}"),
    }
}

/// Calling a `@traced` fn succeeds and returns the correct value.
#[test]
fn traced_fn_call_returns_correctly() {
    let mut interp = interp_for(
        "@traced fn add(a: int, b: int) to int { return a + b }\nfn main() to int { return add(3, 4) }",
    );
    let result = interp.call("main", vec![]).expect("call main");
    assert_eq!(result, VoxValue::Int(7), "@traced fn must return its value");
}

/// An untraced function must have `is_traced = false`.
#[test]
fn untraced_fn_value_has_flag_false() {
    let interp = interp_for("fn plain() to int { return 0 }");
    match interp.scope.get("plain") {
        Some(VoxValue::Fn { is_traced, .. }) => {
            assert!(!is_traced, "plain fn must have is_traced = false");
        }
        other => panic!("expected Fn, got {other:?}"),
    }
}
