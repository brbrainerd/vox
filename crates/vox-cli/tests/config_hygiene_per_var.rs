/// Integration test: per-var baseline keys for `env-var-not-in-registry`.
///
/// Proves that a file-level baseline entry for VAR_A does NOT grandfather VAR_B
/// in the same file — i.e., fine-grained `check|file|env_var` keys are used for
/// this check, not the coarse `check|file` keys used by other checks.
use std::collections::BTreeSet;
use vox_cli::commands::ci::config_hygiene::{
    baseline_key, check_env_reads_registered, unbaselined,
};

#[test]
fn per_var_baseline_new_unregistered_var_in_dirty_file_fails() {
    // Two unregistered vars in the same file.
    let src = r#"
        let a = std::env::var("VOX_ALPHA_UNREGISTERED").ok();
        let b = std::env::var("VOX_BETA_UNREGISTERED").ok();
    "#;
    let registered = std::collections::HashSet::new(); // nothing registered
    let hits = check_env_reads_registered(src, "crates/some/src/lib.rs", &registered);
    assert_eq!(hits.len(), 2, "expected two violations, got {hits:?}");

    // Simulate baseline that only grandfathers VOX_ALPHA_UNREGISTERED.
    let alpha_hit = hits
        .iter()
        .find(|v| v.message.contains("VOX_ALPHA_UNREGISTERED"))
        .expect("VOX_ALPHA_UNREGISTERED violation not found");

    let mut baseline: BTreeSet<String> = BTreeSet::new();
    baseline.insert(baseline_key(alpha_hit));

    // Key must be fine-grained (contains the var name).
    let alpha_key = baseline_key(alpha_hit);
    assert!(
        alpha_key.contains("VOX_ALPHA_UNREGISTERED"),
        "baseline key should contain the env var name; got: {alpha_key}"
    );

    // Only BETA should be reported as new.
    let new_violations = unbaselined(&hits, &baseline);
    assert_eq!(
        new_violations.len(),
        1,
        "expected exactly one NEW violation (VOX_BETA_UNREGISTERED), got {new_violations:?}"
    );
    assert!(
        new_violations[0].message.contains("VOX_BETA_UNREGISTERED"),
        "the surviving violation should be VOX_BETA_UNREGISTERED"
    );
}

#[test]
fn file_level_baseline_still_works_for_other_checks() {
    use vox_cli::commands::ci::config_hygiene::{Violation, check_no_cwd_relative_contract_paths};

    // Two violations in the same file for a non-env-var check.
    let v1 = Violation {
        check: "no-cwd-relative-contract-path",
        file: "crates/x/src/lib.rs".into(),
        line: 1,
        message: "x".into(),
        env_var: None,
    };
    let v2 = Violation {
        check: "no-cwd-relative-contract-path",
        file: "crates/x/src/lib.rs".into(),
        line: 2,
        message: "x".into(),
        env_var: None,
    };

    // A file-level key should grandfather BOTH violations (coarse behaviour kept).
    let key = baseline_key(&v1);
    assert!(
        !key.contains('|') || key.matches('|').count() == 1,
        "non-env-var key should be check|file with one pipe; got: {key}"
    );

    let mut baseline: BTreeSet<String> = BTreeSet::new();
    baseline.insert(key);

    let all = [v1, v2];
    let new_violations = unbaselined(&all, &baseline);
    assert!(
        new_violations.is_empty(),
        "file-level key should suppress both violations for non-env-var check"
    );

    // Sanity: the source-scan function works too.
    let src = r#"let p = Path::new("contracts/orchestration/circuit-breaker.v1.yaml");"#;
    let hits = check_no_cwd_relative_contract_paths(src, "x.rs");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].env_var, None);
}
