//! Regression test for the `HirModule` `serde_json` round-trip.
//!
//! Background: `HirModule::inferred_types` is a `HashMap<Span, HirType>`. Before the
//! 2026-05-28 fix, this caused `vox build` to panic during the Rust-emit phase
//! (`crates/vox-codegen/src/codegen_rust/emit/main_boot.rs:288`) with:
//!
//! ```text
//! HirModule serializes to JSON ... Error("key must be a string", line: 0, column: 0)
//! ```
//!
//! because `serde_json` requires map keys to be strings and `Span` is a struct.
//!
//! The fix introduced `serialize_with` / `deserialize_with` adapters on the field that
//! use a `"start-end"` string key format. This test guards that the round-trip is
//! lossless and that `vox build`'s embed step (serialize → store as a const &str →
//! deserialize at app boot) cannot regress silently.

use vox_compiler::ast::span::Span;
use vox_compiler::hir::{HirModule, HirType};

fn make_inferred_type_pair(start: usize, end: usize) -> (Span, HirType) {
    // HirType::Named("bool".to_string()) is the simplest concrete variant; any variant exercises the
    // same map-serialization path.
    (Span::new(start, end), HirType::Named("bool".to_string()))
}

#[test]
fn empty_hir_module_roundtrips_through_serde_json() {
    let module = HirModule::default();
    let json = serde_json::to_string(&module).expect("empty HirModule serializes");
    let decoded: HirModule = serde_json::from_str(&json).expect("empty HirModule deserializes");
    assert_eq!(decoded.inferred_types.len(), 0);
}

#[test]
fn hir_module_with_inferred_types_roundtrips_losslessly() {
    let mut module = HirModule::default();
    let pairs = vec![
        make_inferred_type_pair(0, 5),
        make_inferred_type_pair(10, 42),
        make_inferred_type_pair(100, 100), // zero-length span
        make_inferred_type_pair(usize::MAX - 1, usize::MAX), // edge of usize
    ];
    for (span, ty) in &pairs {
        module.inferred_types.insert(*span, ty.clone());
    }

    let json = serde_json::to_string(&module).expect("HirModule serializes");
    // Sanity: the JSON must contain string-form keys, not struct-form `{start,end}`.
    assert!(
        json.contains("\"0-5\""),
        "expected stringified key `0-5` in JSON, got: {json}"
    );

    let decoded: HirModule = serde_json::from_str(&json).expect("HirModule deserializes");
    assert_eq!(
        decoded.inferred_types.len(),
        pairs.len(),
        "decoded inferred_types must preserve every entry"
    );
    for (span, ty) in &pairs {
        let got = decoded.inferred_types.get(span).expect("span preserved");
        assert_eq!(got, ty, "type for {:?} round-tripped equal", span);
    }
}

#[test]
fn deserialize_rejects_malformed_span_keys() {
    // Construct JSON with a malformed key and assert we get a useful error rather than a panic.
    let bad = r#"{
        "imports": [],
        "rust_imports": [],
        "functions": [],
        "types": [],
        "tests": [],
        "examples": [],
        "foralls": [],
        "endpoint_fns": [],
        "tables": [],
        "indexes": [],
        "collections": [],
        "vector_indexes": [],
        "search_indexes": [],
        "mcp_tools": [],
        "mcp_resources": [],
        "agents": [],
        "environments": [],
        "components": [],
        "client_routes": [],
        "url_decls": [],
        "state_machines": [],
        "fragments": [],
        "reactive_modules": [],
        "forms": [],
        "back_button": null,
        "deep_link": null,
        "push": null,
        "token_decls": [],
        "route_ids": [],
        "inferred_types": { "not-a-valid-span": { "Named": "bool" } },
        "legacy_ast_nodes": []
    }"#;
    let err = serde_json::from_str::<HirModule>(bad).expect_err("malformed key must error");
    let msg = err.to_string();
    assert!(
        msg.contains("invalid") || msg.contains("not-a-valid-span"),
        "error message should describe the bad key: {msg}"
    );
}

/// Mirrors what `crates/vox-codegen/src/codegen_rust/emit/main_boot.rs:288` does — serialize a
/// `HirModule` to a JSON string and verify it does not panic. Regression gate for the original bug.
#[test]
fn main_boot_serialize_pattern_does_not_panic() {
    let mut module = HirModule::default();
    module
        .inferred_types
        .insert(Span::new(7, 14), HirType::Named("bool".to_string()));
    // The exact call shape from main_boot.rs:287
    let _ = serde_json::to_string(&module).expect("HirModule serializes (regression gate)");
}
