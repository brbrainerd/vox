/// Integration tests for the Vox type checker.
///
/// These tests verify that the type checker correctly:
/// - Catches undefined variables
/// - Enforces mutability (can't assign to immutable bindings)
/// - Detects non-exhaustive match expressions
/// - Validates return types
/// - Handles component return type requirements

use vox_lexer::cursor::lex;
use vox_parser::parser::parse;
use vox_typeck::typecheck_module;
use vox_typeck::diagnostics::Severity;

/// Helper: parse a Vox source string and return diagnostics.
fn check(src: &str) -> Vec<vox_typeck::Diagnostic> {
    let tokens = lex(src);
    let module = parse(tokens).expect("Source should parse without errors");
    typecheck_module(&module)
}

/// Helper: get only error-severity diagnostics.
fn errors(src: &str) -> Vec<vox_typeck::Diagnostic> {
    check(src)
        .into_iter()
        .filter(|d| d.severity == Severity::Error)
        .collect()
}

/// Helper: get only warning-severity diagnostics.
fn warnings(src: &str) -> Vec<vox_typeck::Diagnostic> {
    check(src)
        .into_iter()
        .filter(|d| d.severity == Severity::Warning)
        .collect()
}

// ── Undefined variable detection ─────────────────────────────

#[test]
fn undefined_variable_in_function() {
    let errs = errors(r#"
fn greet() to Unit:
    print(xyz)
"#);
    assert!(
        errs.iter().any(|d| d.message.contains("Undefined variable 'xyz'")),
        "Should catch undefined variable 'xyz', got: {:?}", errs
    );
}

#[test]
fn defined_parameter_no_error() {
    let errs = errors(r#"
fn greet(name: str) to Unit:
    print(name)
"#);
    assert!(errs.is_empty(), "Should have no errors for defined parameter, got: {:?}", errs);
}

#[test]
fn defined_let_binding_no_error() {
    let errs = errors(r#"
fn compute() to int:
    let x = 42
    ret x
"#);
    assert!(errs.is_empty(), "Should have no errors for defined let binding, got: {:?}", errs);
}

// ── Mutability enforcement ──────────────────────────────────

#[test]
fn assign_to_immutable_variable() {
    let errs = errors(r#"
fn counter() to Unit:
    let x = 0
    x = 1
"#);
    assert!(
        errs.iter().any(|d| d.message.contains("Cannot assign to immutable variable 'x'")),
        "Should catch assignment to immutable variable, got: {:?}", errs
    );
}

#[test]
fn assign_to_mutable_variable_ok() {
    let errs = errors(r#"
fn counter() to Unit:
    let mut x = 0
    x = 1
"#);
    assert!(
        errs.is_empty(),
        "Should allow assignment to mutable variable, got: {:?}", errs
    );
}

// ── Match exhaustiveness ────────────────────────────────────

#[test]
fn exhaustive_match_no_error() {
    let errs = errors(r#"
type Color =
    | Red
    | Green
    | Blue

fn describe(c: Color) to str:
    match c:
        Red -> "red"
        Green -> "green"
        Blue -> "blue"
"#);
    assert!(errs.is_empty(), "Exhaustive match should have no errors, got: {:?}", errs);
}

#[test]
fn non_exhaustive_match_error() {
    let errs = errors(r#"
type Color =
    | Red
    | Green
    | Blue

fn describe(c: Color) to str:
    match c:
        Red -> "red"
        Green -> "green"
"#);
    assert!(
        errs.iter().any(|d| d.message.contains("Non-exhaustive match") && d.message.contains("Blue")),
        "Should catch missing Blue variant, got: {:?}", errs
    );
}

#[test]
fn match_with_wildcard_is_exhaustive() {
    let errs = errors(r#"
type Color =
    | Red
    | Green
    | Blue

fn describe(c: Color) to str:
    match c:
        Red -> "red"
        _ -> "other"
"#);
    assert!(errs.is_empty(), "Wildcard match should be exhaustive, got: {:?}", errs);
}

// ── ADT constructors ───────────────────────────────────────

#[test]
fn adt_constructor_is_defined() {
    let errs = errors(r#"
type Shape =
    | Circle(r: float)
    | Point

fn make_circle() to Shape:
    ret Circle(3.14)
"#);
    assert!(errs.is_empty(), "ADT constructors should be in scope, got: {:?}", errs);
}

// ── Component return type checking ──────────────────────────

#[test]
fn component_with_element_return_no_warning() {
    let warns = warnings(r#"
@component fn Button() to Element:
    <button>"Click"</button>
"#);
    assert!(warns.is_empty(), "Component returning Element should have no warnings, got: {:?}", warns);
}

// ── Built-in functions ──────────────────────────────────────

#[test]
fn builtin_print_is_defined() {
    let errs = errors(r#"
fn hello() to Unit:
    print("Hello World")
"#);
    assert!(errs.is_empty(), "print should be a known builtin, got: {:?}", errs);
}

#[test]
fn builtin_ok_error_constructors() {
    let errs = errors(r#"
fn succeed() to Result[str]:
    ret Ok("success")
"#);
    assert!(errs.is_empty(), "Ok should be a known builtin constructor, got: {:?}", errs);
}

// ── Actor declaration checking ──────────────────────────────

#[test]
fn actor_handler_checks_body() {
    let errs = errors(r#"
actor Worker:
    on receive(msg: str) to Unit:
        print(msg)
"#);
    assert!(errs.is_empty(), "Actor handler with valid body should be clean, got: {:?}", errs);
}

#[test]
fn actor_handler_undefined_var() {
    let errs = errors(r#"
actor Worker:
    on receive(msg: str) to Unit:
        print(unknown_var)
"#);
    assert!(
        errs.iter().any(|d| d.message.contains("Undefined variable 'unknown_var'")),
        "Should catch undefined variable in actor handler, got: {:?}", errs
    );
}
