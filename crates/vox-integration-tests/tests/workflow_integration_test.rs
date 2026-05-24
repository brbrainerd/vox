#![allow(missing_docs)]

use vox_compiler::lexer::cursor::lex;
use vox_compiler::parser::parse;
use vox_compiler::pipeline::run_frontend_str;
use vox_compiler::typeck::diagnostics::TypeckSeverity;
use vox_compiler::typeck::typecheck_module;

fn check_src(src: &str) -> Vec<vox_compiler::typeck::Diagnostic> {
    let tokens = lex(src);
    let module = parse(tokens).expect("Source should parse without generic errors");
    typecheck_module(&module, src)
}

fn errors(src: &str) -> Vec<vox_compiler::typeck::Diagnostic> {
    check_src(src)
        .into_iter()
        .filter(|d| d.severity == TypeckSeverity::Error)
        .collect()
}

fn warnings(src: &str) -> Vec<vox_compiler::typeck::Diagnostic> {
    check_src(src)
        .into_iter()
        .filter(|d| d.severity == TypeckSeverity::Warning)
        .collect()
}

// ── Durability grammar acceptance (ADR-041, supersedes ADR-028) ───────────────
// `activity` and `workflow` are stable public-grammar keywords backed by a real
// durable runtime (ADR-019 + ADR-041). The pipeline must NOT emit ADR-028-style
// reservation errors for them. These tests guard against regression of the gate
// lift that landed alongside ADR-041's enactment.

fn has_e028_reservation_error(result: &vox_compiler::pipeline::FrontendResult) -> bool {
    result.diagnostics.iter().any(|d| {
        d.code.as_deref() == Some("E028") || d.message.contains("reserved for a future release")
    })
}

#[test]
fn activity_keyword_accepted_by_pipeline_adr041() {
    let src = r#"
activity send_email(recipient: str, subject: str) to Result[str] {
    return Ok("ok")
}
"#;
    let result = run_frontend_str(src, "test.vox").expect("pipeline should not hard-fail");
    assert!(
        !has_e028_reservation_error(&result),
        "ADR-041: `activity` is a stable keyword; no E028 reservation error expected. diags: {:?}",
        result.diagnostics
    );
}

#[test]
fn workflow_keyword_accepted_by_pipeline_adr041() {
    let src = r#"
workflow main_flow() to Result[str] {
    return Ok("done")
}
"#;
    let result = run_frontend_str(src, "test.vox").expect("pipeline should not hard-fail");
    assert!(
        !has_e028_reservation_error(&result),
        "ADR-041: `workflow` is a stable keyword; no E028 reservation error expected. diags: {:?}",
        result.diagnostics
    );
}

#[test]
fn activity_and_workflow_together_accepted_by_pipeline_adr041() {
    let src = r#"
activity process_data(data: str) to Result[str] {
    return Ok(data)
}

workflow run_pipeline() to Result[str] {
    let result = process_data("test")
    return result
}
"#;
    let result = run_frontend_str(src, "test.vox").expect("pipeline should not hard-fail");
    assert!(
        !has_e028_reservation_error(&result),
        "ADR-041: `activity` + `workflow` are stable keywords; no E028 reservation error expected. diags: {:?}",
        result.diagnostics
    );
}

// ── `with` operator on plain `fn` contexts ────────────────────────────────────

#[test]
fn test_with_operator_associativity() {
    let src = r#"
fn f() to Result[int] {
    let x = Ok(1) with { meta: "data" }
    x
}
"#;
    let errs = errors(src);
    assert!(
        errs.is_empty(),
        "`with` applies to Result operands; Ok(1) with options should typecheck, got: {:?}",
        errs
    );
}

#[test]
fn test_with_non_record_options_error() {
    let src = r#"
fn f() to int {
    let x = 1 with "invalid"
    x
}
"#;
    let errs = errors(src);
    assert!(
        !errs.is_empty(),
        "Using 'with' with a non-record should produce error"
    );
    assert!(errs[0].message.contains("'with' options must be a record"));
}

#[test]
fn test_with_unknown_option_key_warning() {
    let src = r#"
fn f() to int {
    let x = 1 with { unknown_key: 42 }
    x
}
"#;
    let warns = warnings(src);
    assert!(
        !warns.is_empty(),
        "Unknown 'with' option key should produce warning"
    );
    assert!(
        warns[0].message.contains("Unknown 'with' option"),
        "Got: {}",
        warns[0].message
    );
}

#[test]
fn test_with_wrong_option_type_warning() {
    let src = r#"
fn f() to int {
    let x = 1 with { retries: "not_a_number" }
    x
}
"#;
    let warns = warnings(src);
    assert!(
        !warns.is_empty(),
        "Wrong type for 'retries' should produce warning"
    );
    assert!(
        warns[0].message.contains("retries"),
        "Got: {}",
        warns[0].message
    );
    assert!(
        warns[0].message.contains("Int"),
        "Should mention expected type Int"
    );
}

// ── Table / Index type checking ───────────────────────────────────────────────

#[test]
fn test_table_registration_no_errors() {
    let src = r#"
@table type Task {
    title: str
    done: bool
    priority: int
}
"#;
    let errs = errors(src);
    assert!(
        errs.is_empty(),
        "Valid table should have no errors, got: {:?}",
        errs
    );
}

#[test]
fn test_index_on_known_table_no_errors() {
    let src = r#"
@table type Task {
    title: str
    done: bool
}

@index Task.by_done on (done)
"#;
    let errs = errors(src);
    assert!(
        errs.is_empty(),
        "Index on known table should have no errors, got: {:?}",
        errs
    );
}

#[test]
fn test_index_on_unknown_table_error() {
    let src = r#"
@index Missing.by_name on (name)
"#;
    let errs = errors(src);
    assert!(
        !errs.is_empty(),
        "Index on unknown table should produce an error"
    );
    assert!(
        errs[0].message.contains("unknown table 'Missing'"),
        "Error message: {}",
        errs[0].message
    );
}

// ── Argument / generic type checking ─────────────────────────────────────────

#[test]
fn test_arg_type_mismatch_error() {
    let src = r#"
fn add(a: int, b: int) to int {
    return a
}

fn main() to int {
    return add(1, "str")
}
"#;
    let errs = errors(src);
    assert!(
        !errs.is_empty(),
        "Type mismatch in args should produce error"
    );
    assert!(
        errs[0].message.contains("Argument type mismatch"),
        "Got: {}",
        errs[0].message
    );
}

#[test]
fn test_arg_count_mismatch_error() {
    let src = r#"
fn add(a: int, b: int) to int {
    return a
}

fn main() to int {
    return add(1)
}
"#;
    let errs = errors(src);
    assert!(!errs.is_empty(), "Arg count mismatch should produce error");
    assert!(
        errs[0].message.contains("Argument count mismatch"),
        "Got: {}",
        errs[0].message
    );
}

#[test]
fn test_generic_type_mismatch() {
    let src = r#"
fn id<T>(x: T) to T {
    return x
}

fn main() to int {
    let s: str = id(1)
    return 0
}
"#;
    let errs = errors(src);
    assert!(
        !errs.is_empty(),
        "Generic type mismatch should produce error"
    );
    let msg = &errs[0].message;
    assert!(
        msg.contains("mismatch") || msg.contains("Incompatible"),
        "Got: {}",
        msg
    );
}

#[test]
fn test_generic_identity_works() {
    let src = r#"
fn id<T>(x: T) to T {
    return x
}

fn main() to int {
    let i: int = id(1)
    return i
}
"#;
    let errs = errors(src);
    assert!(
        errs.is_empty(),
        "Valid generic identity call should pass type check, got: {:?}",
        errs
    );
}
