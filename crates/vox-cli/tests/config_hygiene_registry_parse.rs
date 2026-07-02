//! Tests for the serde-based registry loader (Phase 1).
use std::io::Write;
use tempfile::NamedTempFile;
use vox_cli_ci::config_hygiene::parse_registry_file;

#[test]
fn malformed_yaml_registry_returns_err_not_empty_set() {
    let mut f = NamedTempFile::new().unwrap();
    writeln!(f, "knobs: [invalid yaml {{{{").unwrap();
    // When the registry file is malformed, config-hygiene must fail hard (not silently
    // return an empty set and skip all checks).
    let result = parse_registry_file(f.path());
    assert!(
        result.is_err(),
        "malformed YAML must return Err, not empty set"
    );
}

#[test]
fn valid_registry_row_is_parsed() {
    let yaml = "schema_version: \"2\"\nknobs:\n  - env_var: VOX_TEST_FOO\n    description: test\n";
    let mut f = NamedTempFile::new().unwrap();
    f.write_all(yaml.as_bytes()).unwrap();
    let result = parse_registry_file(f.path());
    assert!(result.is_ok());
    let rows = result.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].env_var, "VOX_TEST_FOO");
}
