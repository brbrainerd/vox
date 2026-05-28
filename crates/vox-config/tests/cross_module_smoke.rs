//! Cross-module integration smoke (`vox-config`): paths + env parsing + bootstrap constants.

use vox_config::{bootstrap_inference, env_parse, operator_registry, paths, routing_migration};

#[test]
fn paths_constants_and_join_helpers_resolve() {
    assert_eq!(paths::APP_DIR_NAME, "vox");
    assert_eq!(paths::DEFAULT_DB_FILENAME, "vox.db");

    let tmp = tempfile::tempdir().expect("tempdir");
    let repo = tmp.path();
    let rid = "repo-smoke";
    let tooling = paths::repo_tooling_cache_dir(repo, rid);
    assert!(tooling.starts_with(repo));
    assert!(tooling.to_string_lossy().contains(rid));

    let home = paths::user_home_dir();
    let dot = paths::dot_vox_user_dir();
    assert!(dot.starts_with(home));
}

#[test]
fn env_parse_helpers_without_mutating_env() {
    assert_eq!(env_parse::parse_u64_opt(Some(" 42 "), 0), 42);
    assert_eq!(env_parse::parse_u64_opt(Some("bad"), 99), 99);
    assert_eq!(env_parse::parse_usize_opt(None, 3), 3);
}

#[test]
fn bootstrap_inference_pins_known_aliases() {
    assert_eq!(bootstrap_inference::OPENROUTER_AUTO, "openrouter/auto");
    assert_eq!(
        bootstrap_inference::NLI_FALLBACK,
        "google/gemini-3.1-flash-lite"
    );
}

#[test]
fn operator_registry_exports_non_empty_env_catalog() {
    let names = operator_registry::all_operator_env_names();
    assert!(
        names.contains(&"VOX_SEARCH_POLICY_VERSION"),
        "expected at least one known operator env name in catalog"
    );
}

#[test]
fn routing_migration_raw_detects_enforce_phases() {
    assert!(routing_migration::secrets_cutover_blocks_legacy_env_raw(
        "enforce"
    ));
    assert!(routing_migration::secrets_cutover_blocks_legacy_env_raw(
        "Decommission"
    ));
    assert!(!routing_migration::secrets_cutover_blocks_legacy_env_raw(
        "warn"
    ));
}

#[test]
fn repo_backend_artifact_dir_under_dot_vox() {
    let tmp = tempfile::tempdir().unwrap();
    let p = paths::repo_backend_artifact_dir(tmp.path());
    assert!(p.ends_with("backend-artifact"));
    assert!(p.starts_with(tmp.path()));
}
