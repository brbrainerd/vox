//! Integration tests for tier validation against the distribution SSOT.

use voxup::profiles::{validate_tier, PROFILES_YAML};

#[test]
fn unknown_tier_errors_with_valid_tier_list() {
    let err = validate_tier(PROFILES_YAML, "bogus")
        .expect_err("unknown tier must return Err");
    assert!(err.contains("minimal"), "error must name 'minimal': {err}");
    assert!(err.contains("default"), "error must name 'default': {err}");
    assert!(err.contains("full"), "error must name 'full': {err}");
    assert!(err.contains("bogus"), "error must echo back the bad value: {err}");
}

#[test]
fn known_tiers_are_accepted() {
    for tier in &["minimal", "default", "full"] {
        validate_tier(PROFILES_YAML, tier)
            .unwrap_or_else(|e| panic!("tier '{tier}' must be valid: {e}"));
    }
}

#[test]
fn error_does_not_contain_yaml_noise() {
    let err = validate_tier(PROFILES_YAML, "xyzzy")
        .expect_err("unknown tier must return Err");
    assert!(
        !err.contains("schema_version"),
        "error should not leak YAML internals: {err}"
    );
}
