//! Tests for the rename registry loader and alias resolution.
//!
//! Implementation lives in `crates/vox-compiler/src/parser/renames.rs` (added in Task 3).
//! This test file is written first (TDD) and will fail to compile until that module exists.

use vox_compiler::parser::renames::RenameRegistry;

#[test]
fn registry_loads_from_canonical_path() {
    let registry =
        RenameRegistry::load_canonical().expect("should load contracts/naming/renames.v1.json");
    // Empty registry is valid (VUV-9 ships an empty registry; entries are added in later phases).
    assert_eq!(registry.entries().count(), 0);
}

#[test]
fn registry_rejects_duplicate_from_keys() {
    let json = r#"{
        "version": 1,
        "entries": [
          { "from": "Box", "to": "panel", "kind": "primitive", "since": "0.5.0" },
          { "from": "Box", "to": "container", "kind": "primitive", "since": "0.5.0" }
        ]
    }"#;
    let result = RenameRegistry::parse_json(json);
    assert!(result.is_err(), "duplicate `from` keys must be rejected");
}

#[test]
fn registry_rejects_alias_chain() {
    // `from` cannot itself be a `to` of another entry — no chains, only direct mappings.
    let json = r#"{
        "version": 1,
        "entries": [
          { "from": "Box",       "to": "container", "kind": "primitive", "since": "0.5.0" },
          { "from": "container", "to": "panel",     "kind": "primitive", "since": "0.5.0" }
        ]
    }"#;
    let result = RenameRegistry::parse_json(json);
    assert!(result.is_err(), "alias chains must be rejected");
}

#[test]
fn registry_accepts_empty_entries() {
    let json = r#"{ "version": 1, "entries": [] }"#;
    let result = RenameRegistry::parse_json(json);
    assert!(result.is_ok(), "empty registry must be valid");
}

#[test]
fn registry_rejects_unsupported_version() {
    let json = r#"{ "version": 99, "entries": [] }"#;
    let result = RenameRegistry::parse_json(json);
    assert!(result.is_err(), "version != 1 must be rejected");
}

use vox_compiler::parser;

#[test]
fn deprecated_primitive_resolves_with_warning() {
    let registry_json = r#"{
        "version": 1,
        "entries": [
          { "from": "Box", "to": "panel", "kind": "primitive", "since": "0.5.0" }
        ]
    }"#;
    let registry = RenameRegistry::parse_json(registry_json).unwrap();

    let source = "component App() { view: Box() { } }";
    let result = parser::parse_with_registry(source, &registry).expect("source should parse");

    // The resolved primitive should be `panel`, not `Box`.
    assert!(
        result.uses_primitive("panel"),
        "expected `panel` in resolved primitives, got: {:?}",
        result
    );
    assert!(
        !result.uses_primitive("Box"),
        "expected `Box` to have been resolved away, got: {:?}",
        result
    );

    // Exactly one deprecation warning, citing all three pieces.
    let warnings = result.warnings();
    assert_eq!(
        warnings.len(),
        1,
        "expected exactly one warning, got {:?}",
        warnings
    );
    let msg = &warnings[0].message;
    assert!(
        msg.contains("Box"),
        "warning should name old name `Box`, got: {}",
        msg
    );
    assert!(
        msg.contains("panel"),
        "warning should name new name `panel`, got: {}",
        msg
    );
    assert!(
        msg.contains("0.5.0"),
        "warning should cite version `0.5.0`, got: {}",
        msg
    );
    assert!(
        msg.contains("vox migrate names"),
        "warning should suggest running `vox migrate names`, got: {}",
        msg
    );
}

#[test]
fn registry_rejects_empty_from() {
    let json = r#"{
        "version": 1,
        "entries": [
          { "from": "", "to": "panel", "kind": "primitive", "since": "0.5.0" }
        ]
    }"#;
    assert!(
        RenameRegistry::parse_json(json).is_err(),
        "empty `from` must be rejected"
    );
}

#[test]
fn registry_rejects_self_rename() {
    let json = r#"{
        "version": 1,
        "entries": [
          { "from": "Box", "to": "Box", "kind": "primitive", "since": "0.5.0" }
        ]
    }"#;
    assert!(
        RenameRegistry::parse_json(json).is_err(),
        "self-rename (from == to) must be rejected"
    );
}
