//! Task 3.2: verify that the 5 user-reachable panic sites in the TS emitter now
//! return/collect diagnostics instead of panicking.
//!
//! Sites converted:
//! 1. `channels.rs:42`  — panic reading user-authored channels.v1.yaml
//! 2. `channels.rs:43`  — panic parsing user-authored channels.v1.yaml
//! 3. `openapi_emit.rs:67`  — `.expect()` on OpenAPI JSON serialization
//! 4. `route_manifest.rs:398` — `.unwrap()` on route manifest JSON serialization
//! 5. `library_package_emit.rs:55` — `.expect()` on library package.json serialization

use std::path::Path;
use vox_compiler::typeck::diagnostics::codes;

// ─── Site 1 & 2: channels.rs bad-file / bad-YAML ────────────────────────────

/// Site 1: `load_channel_contract_from_path` with a non-existent file previously
/// hit `unwrap_or_else(|e| panic!(...))` on the `read_to_string` call.
///
/// POST-FIX: returns `Err(Diagnostic)` with code `vox/codegen-ts/unsupported`.
#[test]
fn site1_channels_missing_file_returns_diagnostic() {
    let result = vox_codegen_ts::channels::load_channel_contract_from_path(Path::new(
        "/nonexistent/path/channels.v1.yaml",
    ));
    let diag = result.expect_err("expected Err(Diagnostic) for missing file");
    assert_eq!(
        diag.code.as_deref(),
        Some(codes::CODEGEN_TS_UNSUPPORTED),
        "diagnostic code must be {}",
        codes::CODEGEN_TS_UNSUPPORTED
    );
    assert!(
        diag.message.contains("TypeScript emitter"),
        "message should identify the TS emitter, got: {:?}",
        diag.message
    );
}

/// Site 2: `parse_channel_contract` with invalid YAML previously hit
/// `serde_yaml::from_str(...).unwrap_or_else(|e| panic!(...))`.
///
/// POST-FIX: returns `Err(Diagnostic)` with code `vox/codegen-ts/unsupported`.
#[test]
fn site2_channels_bad_yaml_returns_diagnostic() {
    // Malformed YAML: tabs not allowed as indentation in YAML 1.1/1.2
    let bad_yaml = "schema_version: 1\nchannels:\n\t- bad_tab_indent";
    let result = vox_codegen_ts::channels::parse_channel_contract(bad_yaml);
    let diag = result.expect_err("expected Err(Diagnostic) for bad YAML");
    assert_eq!(
        diag.code.as_deref(),
        Some(codes::CODEGEN_TS_UNSUPPORTED),
        "diagnostic code must be {}",
        codes::CODEGEN_TS_UNSUPPORTED
    );
    assert!(
        diag.message.contains("channel contract"),
        "message should mention channel contract, got: {:?}",
        diag.message
    );
}

// ─── Site 3: openapi_emit.rs serialization ───────────────────────────────────

/// Site 3: `emit_from_contract` (via `generate_openapi`) previously called
/// `.expect("OpenAPI emit must serialize")` which would panic if serde_json
/// ever fails to serialize the `Value::Object`.
///
/// POST-FIX: the `.expect()` is replaced with `unwrap_or_else` emitting a
/// diagnostic comment containing the unsupported code.
///
/// Since `Value::Object` is always serializable, we verify the happy path still
/// produces valid JSON.
#[test]
fn site3_openapi_emit_does_not_panic_on_empty_module() {
    use vox_compiler::hir::HirModule;

    let hir = HirModule::default();

    // Should not panic; an empty module produces valid JSON
    let output = vox_codegen_ts::openapi_emit::generate_openapi(&hir, "test-pkg", "0.0.1");
    assert!(
        output.contains("openapi"),
        "output should contain OpenAPI field"
    );
}

// ─── Site 4: route_manifest.rs serialization ─────────────────────────────────

/// Site 4: `emit_contract_route_manifest` previously called `.unwrap()` on
/// `serde_json::to_string_pretty(...)` which would panic if serialization failed.
///
/// POST-FIX: replaced with `unwrap_or_else` returning a diagnostic comment.
///
/// We verify the function returns `None` (no routes) without panicking.
#[test]
fn site4_route_manifest_does_not_panic_on_empty_web_ir() {
    use vox_codegen::web_ir::WebIrModule;
    use vox_compiler::hir::HirModule;

    let web = WebIrModule::default();
    let hir = HirModule::default();

    // Should not panic even on an empty module
    let result = vox_codegen_ts::route_manifest::emit_route_manifest_json(&web, &hir);
    // Empty module has no routes → returns None (no manifest)
    assert!(
        result.is_none(),
        "empty module should return None, got: {result:?}"
    );
}

// ─── Site 5: library_package_emit.rs serialization ───────────────────────────

/// Site 5: `emit_library_package_json` previously called
/// `.expect("library package.json serializes")` which would panic if
/// serde_json serialization failed.
///
/// POST-FIX: replaced with `unwrap_or_else` returning a diagnostic comment.
///
/// We verify the happy path produces valid JSON.
#[test]
fn site5_library_package_json_does_not_panic() {
    use vox_codegen_ts::library_package_emit::{LibraryPackageConfig, emit_library_package_json};

    let config = LibraryPackageConfig {
        has_vox_client: true,
        has_types: true,
        has_schemas: false,
        has_openapi: false,
        has_schema_ts: false,
        component_names: vec![],
    };

    // Should not panic
    let output = emit_library_package_json(config);
    // Verify it's valid JSON
    let parsed: serde_json::Value =
        serde_json::from_str(&output).expect("emit_library_package_json must produce valid JSON");
    assert!(
        parsed.get("name").is_some(),
        "package.json must have a name field"
    );
}
