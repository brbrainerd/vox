use vox_codegen::codegen_ts::emitter::generate;
use vox_compiler::{hir::lower_module, lexer::cursor::lex, parser::parse};

fn try_emit(src: &str) -> Result<String, String> {
    let m = parse(lex(src)).map_err(|e| format!("{e:?}"))?;
    let hir = lower_module(&m);
    let out = generate(&hir)?;
    Ok(out
        .files
        .iter()
        .map(|(_, c)| c.clone())
        .collect::<Vec<_>>()
        .join("\n"))
}

#[test]
fn route_with_loader_no_pending_fails() {
    let src = r#"
@query fn load() to int { return 1 }
component X() { view: column(raw_class="x") { "x" } }
routes {
    "/x" to X with (loader: load)
}
"#;
    let err = try_emit(src).expect_err("must fail without pending");
    assert!(
        err.contains("validate.route.missing_pending"),
        "expected validate.route.missing_pending; got: {err}"
    );
}

#[test]
fn route_with_loader_pending_but_no_error_fails() {
    let src = r#"
@query fn load() to int { return 1 }
component X() { view: column(raw_class="x") { "x" } }
component XLoading() { view: column(raw_class="l") { "..." } }
routes {
    "/x" to X with (loader: load, pending: XLoading)
}
"#;
    let err = try_emit(src).expect_err("must fail without error component");
    assert!(
        err.contains("validate.route.missing_error"),
        "expected validate.route.missing_error; got: {err}"
    );
}

#[test]
fn route_with_loader_pending_and_error_passes() {
    let src = r#"
@query fn load() to int { return 1 }
component X() { view: column(raw_class="x") { "x" } }
component XLoading() { view: column(raw_class="l") { "..." } }
component XError() { view: column(raw_class="e") { "err" } }
routes {
    "/x" to X with (loader: load, pending: XLoading, error: XError)
}
"#;
    let _ts = try_emit(src).expect("should pass with all three");
}

#[test]
fn route_without_loader_does_not_require_pending_or_error() {
    let src = r#"
component X() { view: column(raw_class="x") { "x" } }
routes {
    "/x" to X
}
"#;
    let _ts = try_emit(src).expect("no loader = no requirement");
}
