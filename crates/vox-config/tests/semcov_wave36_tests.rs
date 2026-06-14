//! Adversarial tests for vox-config: env parsing, path resolution, rollout flags,
//! routing policy, project manifest parsing, and secrets cutover migration helpers.
//!
//! Module: semcov_wave36_tests

use std::path::Path;
use std::sync::Mutex;

/// Serialize all env-mutating tests so they don't race each other.
static ENV_LOCK: Mutex<()> = Mutex::new(());

// ─── env_parse ───────────────────────────────────────────────────────────────

#[test]
fn parse_u64_opt_empty_string_falls_back_to_default() {
    // Catches: "" being parsed as 0 instead of triggering the default branch
    assert_eq!(vox_config::env_parse::parse_u64_opt(Some(""), 99), 99);
}

#[test]
fn parse_u64_opt_whitespace_only_falls_back_to_default() {
    // Catches: "  " surviving trim and being misinterpreted as 0
    assert_eq!(vox_config::env_parse::parse_u64_opt(Some("   "), 42), 42);
}

#[test]
fn parse_u64_opt_negative_string_falls_back_to_default() {
    // Catches: signed-string "-1" silently wrapping to u64::MAX
    assert_eq!(vox_config::env_parse::parse_u64_opt(Some("-1"), 7), 7);
}

#[test]
fn parse_u64_opt_hex_string_falls_back_to_default() {
    // Catches: "0xFF" being accepted as a valid integer via some parse path
    assert_eq!(vox_config::env_parse::parse_u64_opt(Some("0xFF"), 3), 3);
}

#[test]
fn parse_usize_opt_overflow_string_falls_back_to_default() {
    // Catches: very-large number exceeding usize wrapping silently
    let huge = "999999999999999999999999999999";
    assert_eq!(vox_config::env_parse::parse_usize_opt(Some(huge), 5), 5);
}

#[test]
fn env_u64_absent_var_returns_default() {
    // Catches: missing env var returning 0 instead of the caller's default
    let _g = ENV_LOCK.lock().unwrap();
    unsafe { std::env::remove_var("_VOX_TEST_U64_ABSENT_4287") };
    assert_eq!(
        vox_config::env_parse::env_u64("_VOX_TEST_U64_ABSENT_4287", 1234),
        1234
    );
}

#[test]
fn env_duration_from_ms_uses_zero_ms_correctly() {
    // Catches: zero-ms being treated as "use default" rather than a valid 0-ms duration
    let _g = ENV_LOCK.lock().unwrap();
    unsafe { std::env::set_var("_VOX_TEST_DUR_MS_4287", "0") };
    let d = vox_config::env_parse::env_duration_from_ms("_VOX_TEST_DUR_MS_4287", 9999);
    unsafe { std::env::remove_var("_VOX_TEST_DUR_MS_4287") };
    assert_eq!(d.as_millis(), 0);
}

// ─── rollout (env_truthy / flags) ────────────────────────────────────────────

#[test]
fn env_truthy_returns_false_for_absent_var() {
    // Catches: missing env being treated as truthy (would silently disable features)
    let _g = ENV_LOCK.lock().unwrap();
    unsafe { std::env::remove_var("_VOX_TEST_TRUTHY_ABSENT_4287") };
    assert!(!vox_config::rollout::env_truthy(
        "_VOX_TEST_TRUTHY_ABSENT_4287"
    ));
}

#[test]
fn env_truthy_rejects_yes_with_extra_whitespace_correctly() {
    // Catches: " YES " (with leading/trailing spaces) not being trimmed before comparison
    let _g = ENV_LOCK.lock().unwrap();
    unsafe { std::env::set_var("_VOX_TEST_TRUTHY_YES_4287", " YES ") };
    let result = vox_config::rollout::env_truthy("_VOX_TEST_TRUTHY_YES_4287");
    unsafe { std::env::remove_var("_VOX_TEST_TRUTHY_YES_4287") };
    assert!(result, "' YES ' with spaces should be truthy after trim");
}

#[test]
fn env_truthy_rejects_truthy_adjacent_values_like_true1() {
    // Catches: "true1" or "1true" being accepted by a too-broad starts-with check
    let _g = ENV_LOCK.lock().unwrap();
    unsafe { std::env::set_var("_VOX_TEST_TRUTHY_T1_4287", "true1") };
    let result = vox_config::rollout::env_truthy("_VOX_TEST_TRUTHY_T1_4287");
    unsafe { std::env::remove_var("_VOX_TEST_TRUTHY_T1_4287") };
    assert!(!result, "'true1' must not be truthy");
}

#[test]
fn orchestration_lineage_persist_inverts_env_flag() {
    // Catches: double-negation bug where the flag is inverted incorrectly
    let _g = ENV_LOCK.lock().unwrap();
    unsafe { std::env::set_var("VOX_ORCH_LINEAGE_OFF", "1") };
    let result = vox_config::rollout::orchestration_lineage_persist_enabled();
    unsafe { std::env::remove_var("VOX_ORCH_LINEAGE_OFF") };
    assert!(
        !result,
        "lineage should be disabled when VOX_ORCH_LINEAGE_OFF=1"
    );
}

#[test]
fn db_sync_integration_gate_requires_exact_1_not_true() {
    // Catches: gate accepting "true" when it's supposed to require exactly "1"
    let _g = ENV_LOCK.lock().unwrap();
    unsafe { std::env::set_var("VOX_DB_SYNC_INTEGRATION", "true") };
    let result = vox_config::rollout::db_sync_remote_integration_gate_armed();
    unsafe { std::env::remove_var("VOX_DB_SYNC_INTEGRATION") };
    assert!(
        !result,
        "integration gate must require exactly '1', not 'true'"
    );
}

#[test]
fn db_embedded_replica_gate_requires_exact_1() {
    // Catches: gate accepting "yes" or "True" when protocol demands exactly "1"
    let _g = ENV_LOCK.lock().unwrap();
    unsafe { std::env::set_var("VOX_DB_EMBEDDED_REPLICA_INTEGRATION", "yes") };
    let result = vox_config::rollout::db_embedded_replica_integration_gate_armed();
    unsafe { std::env::remove_var("VOX_DB_EMBEDDED_REPLICA_INTEGRATION") };
    assert!(
        !result,
        "embedded replica gate must require exactly '1', not 'yes'"
    );
}

// ─── routing_migration ───────────────────────────────────────────────────────

#[test]
fn secrets_cutover_blocks_enforce_and_decommission_case_insensitive() {
    // Catches: case-sensitive comparison missing mixed-case operator configurations
    assert!(vox_config::routing_migration::secrets_cutover_blocks_legacy_env_raw("ENFORCE"));
    assert!(vox_config::routing_migration::secrets_cutover_blocks_legacy_env_raw("Decommission"));
    assert!(vox_config::routing_migration::secrets_cutover_blocks_legacy_env_raw(" enforce "));
}

#[test]
fn secrets_cutover_does_not_block_migrate_phase() {
    // Catches: "migrate" being conflated with "enforce" and blocking legacy env reads too early
    assert!(!vox_config::routing_migration::secrets_cutover_blocks_legacy_env_raw("migrate"));
    assert!(!vox_config::routing_migration::secrets_cutover_blocks_legacy_env_raw("warning"));
    assert!(!vox_config::routing_migration::secrets_cutover_blocks_legacy_env_raw(""));
}

// ─── routing_policy ──────────────────────────────────────────────────────────

#[test]
fn auto_routing_priority_try_parse_csv_returns_none_for_empty() {
    // Catches: empty string silently installing default weights masking a valid fallback
    assert!(
        vox_config::routing_policy::AutoRoutingPriority::try_parse_csv("").is_none(),
        "empty CSV must return None so callers can reject it"
    );
}

#[test]
fn auto_routing_priority_parse_csv_alias_cost_maps_to_efficiency() {
    // Catches: "cost" alias not wired up, leaving efficiency at default when cost is set
    let p = vox_config::routing_policy::AutoRoutingPriority::parse_csv("cost=77");
    assert_eq!(p.efficiency, 77, "'cost' alias must map to efficiency axis");
}

#[test]
fn auto_routing_priority_parse_csv_alias_quality_maps_to_precision() {
    // Catches: "quality" alias not wired up, leaving precision at default
    let p = vox_config::routing_policy::AutoRoutingPriority::parse_csv("quality=88");
    assert_eq!(
        p.precision, 88,
        "'quality' alias must map to precision axis"
    );
}

#[test]
fn auto_routing_priority_parse_csv_u8_overflow_value_is_rejected() {
    // Catches: "256" being silently truncated to 0 via wrapping u8 cast instead of parse failure
    let p = vox_config::routing_policy::AutoRoutingPriority::parse_csv("efficiency=256,latency=5");
    // 256 does not fit u8; parse::<u8>() fails → efficiency keeps default (25)
    assert_eq!(p.efficiency, 25, "256 must not fit in u8 — keeps default");
    // latency=5 is valid; it should be applied
    assert_eq!(p.latency, 5);
}

#[test]
fn auto_routing_priority_try_parse_csv_wholly_garbage_keys_return_none() {
    // Catches: unknown keys being silently accepted and triggering any_parsed = true
    let result =
        vox_config::routing_policy::AutoRoutingPriority::try_parse_csv("foo=1,bar=2,baz=3");
    assert!(
        result.is_none(),
        "all-unknown keys must produce None even if values are valid u8"
    );
}

#[test]
fn derive_openrouter_route_hint_performance_gives_quality() {
    // Catches: performance→quality mapping being flipped to price
    let hint = vox_config::routing_policy::derive_openrouter_route_hint(
        vox_config::routing_policy::RouteCostPreference::Performance,
    );
    assert_eq!(
        hint,
        vox_config::routing_policy::OpenRouterRouteHint::Quality
    );
}

#[test]
fn derive_openrouter_route_hint_economy_gives_price() {
    // Catches: economy→price mapping being accidentally set to quality or fallback
    let hint = vox_config::routing_policy::derive_openrouter_route_hint(
        vox_config::routing_policy::RouteCostPreference::Economy,
    );
    assert_eq!(hint, vox_config::routing_policy::OpenRouterRouteHint::Price);
}

// ─── paths ───────────────────────────────────────────────────────────────────

#[test]
fn mcp_sessions_dir_is_relative_and_contains_repository_id() {
    // Catches: mcp_sessions_dir returning an absolute path, breaking relative-path consumers
    let dir = vox_config::paths::mcp_sessions_dir("my-repo-123");
    assert!(
        !dir.is_absolute(),
        "mcp_sessions_dir must return a relative path"
    );
    assert!(
        dir.to_string_lossy().contains("my-repo-123"),
        "must contain the repository id"
    );
}

#[test]
fn script_cache_dir_wasi_vs_non_wasi_are_distinct() {
    // Catches: wasi and non-wasi cache dirs colliding (same path returned regardless of flag)
    let normal = vox_config::paths::script_cache_dir(false);
    let wasi = vox_config::paths::script_cache_dir(true);
    assert_ne!(
        normal, wasi,
        "wasi and non-wasi script cache dirs must differ"
    );
    assert!(
        wasi.to_string_lossy().contains("wasi"),
        "wasi cache dir must include 'wasi' in its name"
    );
}

#[test]
fn repo_tooling_cache_dir_is_nested_under_dot_vox() {
    // Catches: cache dir being placed at repo root rather than under .vox/
    let p = vox_config::paths::repo_tooling_cache_dir(Path::new("/workspace"), "proj-abc");
    let s = p.to_string_lossy().replace('\\', "/");
    assert!(
        s.contains("/.vox/cache/repos/proj-abc"),
        "tooling cache must be under .vox/cache/repos/<id>: {s}"
    );
}

#[test]
fn repo_memory_cache_dir_extends_tooling_cache_with_memory_subdir() {
    // Catches: memory dir returning the same path as tooling cache (missing join("memory"))
    let base = vox_config::paths::repo_tooling_cache_dir(Path::new("/ws"), "r");
    let mem = vox_config::paths::repo_memory_cache_dir(Path::new("/ws"), "r");
    assert!(
        mem.starts_with(&base),
        "memory cache must be under tooling cache"
    );
    assert_ne!(
        base, mem,
        "memory cache must be a subdirectory of tooling cache"
    );
    assert_eq!(mem.file_name().unwrap(), "memory");
}

// ─── project_manifest ────────────────────────────────────────────────────────

#[test]
fn project_manifest_load_missing_file_returns_empty_manifest() {
    // Catches: missing Vox.toml returning Err instead of empty-manifest Ok (breaks bootstrapping)
    let p = std::path::PathBuf::from("/tmp/__nonexistent_vox_test_manifest_semcov36__.toml");
    let m = vox_config::project_manifest::ProjectManifest::load(&p)
        .expect("missing file must return Ok with empty manifest");
    assert!(m.workspace.is_none());
    assert!(m.bundle.is_none());
}

#[test]
fn project_manifest_invalid_toml_falls_back_to_defaults() {
    // Catches: malformed TOML propagating a hard error instead of defaulting gracefully
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Vox.toml");
    let mut f = std::fs::File::create(&path).unwrap();
    writeln!(f, "this is {{ not valid toml = [ unclosed").unwrap();
    let m = vox_config::project_manifest::ProjectManifest::load(&path)
        .expect("invalid TOML must not return Err — falls back to defaults");
    // Should be all-None (the unwrap_or_default path)
    assert!(m.workspace.is_none());
    assert!(m.bundle.is_none());
}
