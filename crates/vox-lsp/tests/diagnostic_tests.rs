#![allow(missing_docs)]
#![allow(unsafe_code)]

//! External integration tests for `vox_lsp::validate_document` diagnostics.
//!
//! These tests exercise the full lex → parse → typecheck → mens-warning pipeline
//! through the public crate boundary. Mens-env tests use a mutex to
//! prevent inter-test env pollution when tests run in parallel.

use std::sync::Mutex;

use tower_lsp_server::ls_types::DiagnosticSeverity;
use vox_lsp::validate_document;

/// Serialises all env-mutating tests to prevent cross-test pollution.
static ENV_LOCK: Mutex<()> = Mutex::new(());

// ── No-error cases ───────────────────────────────────────────────────────────

#[test]
fn empty_source_produces_no_diagnostics() {
    let diags = validate_document("");
    assert!(diags.is_empty(), "empty source must be clean: {:?}", diags);
}

#[test]
fn minimal_valid_workflow_produces_no_errors() {
    let diags = validate_document("workflow w() { }");
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
        .collect();
    assert!(
        errors.is_empty(),
        "clean workflow must have no errors: {:?}",
        errors
    );
}

#[test]
fn valid_let_binding_no_diagnostics() {
    let diags = validate_document("fn main() { let x = 42 }");
    let errors: Vec<_> = diags
        .iter()
        .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
        .collect();
    assert!(
        errors.is_empty(),
        "valid let binding must parse clean: {:?}",
        errors
    );
}

// ── Mens activity warnings ────────────────────────────────────────────────────

#[test]
fn mesh_activity_warning_fires_when_mesh_disabled() {
    let _lock = ENV_LOCK.lock().expect("env mutex poisoned");
    // SAFETY: serialised by ENV_LOCK above; no concurrent env mutations.
    unsafe { std::env::set_var("VOX_MESH_ENABLED", "0") };
    let diags = validate_document("workflow w() { mesh_snapshot() }");
    unsafe { std::env::remove_var("VOX_MESH_ENABLED") };

    let has_warn = diags.iter().any(|d| {
        d.severity == Some(DiagnosticSeverity::WARNING) && d.message.contains("Mens activity call")
    });
    assert!(
        has_warn,
        "Expected mens warning when disabled; got: {:?}",
        diags
    );
}

#[test]
fn mesh_activity_no_warning_when_enabled() {
    let _lock = ENV_LOCK.lock().expect("env mutex poisoned");
    // SAFETY: serialised by ENV_LOCK above.
    unsafe { std::env::set_var("VOX_MESH_ENABLED", "1") };
    let diags = validate_document("workflow w() { mesh_snapshot() }");
    unsafe { std::env::remove_var("VOX_MESH_ENABLED") };

    let has_warn = diags.iter().any(|d| {
        d.severity == Some(DiagnosticSeverity::WARNING) && d.message.contains("Mens activity call")
    });
    assert!(
        !has_warn,
        "Expected NO mens warning when enabled; got: {:?}",
        diags
    );
}

#[test]
fn mesh_activity_warning_only_inside_workflow_not_fn() {
    // mesh_* calls outside a `workflow` body must NOT emit warnings.
    let _lock = ENV_LOCK.lock().expect("env mutex poisoned");
    unsafe { std::env::set_var("VOX_MESH_ENABLED", "0") };
    // A regular fn body — should not trigger mens warning even when disabled.
    let diags = validate_document("fn regular() { mesh_snapshot() }");
    unsafe { std::env::remove_var("VOX_MESH_ENABLED") };

    let has_warn = diags.iter().any(|d| {
        d.severity == Some(DiagnosticSeverity::WARNING) && d.message.contains("Mens activity call")
    });
    assert!(
        !has_warn,
        "Mens warning should not fire outside workflow; got: {:?}",
        diags
    );
}

#[test]
fn multiple_mesh_calls_each_produce_a_warning() {
    let _lock = ENV_LOCK.lock().expect("env mutex poisoned");
    unsafe { std::env::set_var("VOX_MESH_ENABLED", "0") };
    let diags = validate_document("workflow w() { mesh_snapshot() mesh_join() }");
    unsafe { std::env::remove_var("VOX_MESH_ENABLED") };

    let warn_count = diags
        .iter()
        .filter(|d| {
            d.severity == Some(DiagnosticSeverity::WARNING)
                && d.message.contains("Mens activity call")
        })
        .count();
    assert!(
        warn_count >= 2,
        "Expected at least 2 mens warnings; got {warn_count}"
    );
}

// ── Diagnostic source labelling ───────────────────────────────────────────────

#[test]
fn all_diagnostics_have_vox_lsp_source() {
    let diags = validate_document("workflow w( { }"); // deliberate syntax error
    for d in &diags {
        let src = d.source.as_deref().unwrap_or("");
        assert_eq!(src, "vox-lsp", "Unexpected source '{src}' in: {:?}", d);
    }
}

// ── Parse error detection ────────────────────────────────────────────────────

#[test]
fn syntax_error_produces_error_diagnostic() {
    // Unclosed paren in ident position → parse error.
    let diags = validate_document("fn broken(");
    let has_error = diags
        .iter()
        .any(|d| d.severity == Some(DiagnosticSeverity::ERROR));
    assert!(
        has_error,
        "malformed source must produce at least one ERROR: {:?}",
        diags
    );
}
