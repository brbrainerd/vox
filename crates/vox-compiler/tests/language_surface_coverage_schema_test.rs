//! CR-F3 (warn-mode scaffold): the language-surface coverage ledger must
//! exist, parse as valid YAML, and validate against its own JSON Schema.
//! This test does NOT check completeness (that's a later hard gate) —
//! only that the file is well-formed and the schema is honest.

use std::fs;

#[test]
fn coverage_ledger_is_valid_yaml_matching_its_schema() {
    let yaml_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contracts/spec/language-surface-coverage.v1.yaml"
    );
    let schema_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contracts/spec/language-surface-coverage.v1.schema.json"
    );

    let yaml_src = fs::read_to_string(yaml_path)
        .unwrap_or_else(|e| panic!("failed to read {yaml_path}: {e}"));
    let schema_src = fs::read_to_string(schema_path)
        .unwrap_or_else(|e| panic!("failed to read {schema_path}: {e}"));

    let doc: serde_yaml::Value =
        serde_yaml::from_str(&yaml_src).expect("ledger must be valid YAML");
    let schema_json: serde_json::Value =
        serde_json::from_str(&schema_src).expect("schema must be valid JSON");

    // Round-trip YAML -> JSON so jsonschema can validate it.
    let doc_json: serde_json::Value =
        serde_json::to_value(&doc).expect("YAML must convert to JSON");

    let validator =
        jsonschema::validator_for(&schema_json).expect("schema itself must compile");
    if let Err(err) = validator.validate(&doc_json) {
        panic!("ledger does not match schema:\n{err}");
    }
}

#[test]
fn coverage_ledger_lists_this_plans_new_productions() {
    let yaml_path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../contracts/spec/language-surface-coverage.v1.yaml"
    );
    let yaml_src = fs::read_to_string(yaml_path).unwrap();
    let doc: serde_yaml::Value = serde_yaml::from_str(&yaml_src).unwrap();
    let productions = doc["productions"]
        .as_sequence()
        .expect("productions must be a list");
    let names: Vec<&str> = productions
        .iter()
        .filter_map(|p| p["name"].as_str())
        .collect();
    for expected in [
        "lexer/unknown-char-token",
        "reader-tolerant-semicolon",
        "reader-tolerant-eq-eq",
        "reader-tolerant-not-eq",
    ] {
        assert!(
            names.contains(&expected),
            "expected production '{expected}' in ledger, got {names:?}"
        );
    }
}
