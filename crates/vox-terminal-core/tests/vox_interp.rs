use vox_terminal_core::vox_interp::eval_line;

#[test]
fn evaluates_vox_expression() {
    let out = eval_line("fn main() -> Int { 40 + 2 }").unwrap();
    assert!(out.contains("42"), "got: {out}");
}

#[test]
fn surfaces_compile_errors() {
    assert!(eval_line("let = ").is_err());
}
