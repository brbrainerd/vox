/// Task 8E: OpenClaw + std.mobile → clean CR-F4 diagnostic in --mode interp
///
/// Native-only namespaces must produce an actionable "use compiled backend" diagnostic
/// instead of a confusing UndefinedVariable("OpenClaw") / UndefinedVariable("Scrape").
use vox_compiler::{
    eval::Interpreter, hir::lower::lower_module, lexer::lex, parser::descent::parse,
};

fn eval_fn_main(src: &str) -> String {
    let tokens = lex(src);
    let module = parse(tokens).expect("parse");
    let lowered = lower_module(&module);
    let mut interp = Interpreter::new(100_000);
    interp.run_module(&lowered).expect("run_module");
    match interp.call("main", vec![]) {
        Ok(_) => "ok".to_string(),
        Err(e) => format!("{e:?}"),
    }
}

#[test]
fn openclaw_call_in_fn_gives_cr_f4_diagnostic() {
    let result = eval_fn_main(
        // vox:skip — OpenClaw requires compiled backend
        "fn main() { let r = OpenClaw.embed(\"hello\") }\n",
    );
    assert!(
        result.contains("compiled builds") || result.contains("native-codegen-only"),
        "Expected CR-F4 diagnostic for OpenClaw, got: {result}"
    );
}

#[test]
fn scrape_call_in_fn_gives_cr_f4_diagnostic() {
    let result = eval_fn_main(
        // vox:skip — Scrape requires compiled backend
        "fn main() { let r = Scrape.fetch(\"https://example.com\") }\n",
    );
    assert!(
        result.contains("compiled builds") || result.contains("native-codegen-only"),
        "Expected CR-F4 diagnostic for Scrape, got: {result}"
    );
}
