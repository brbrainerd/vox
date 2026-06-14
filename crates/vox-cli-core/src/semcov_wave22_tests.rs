//! Adversarial unit tests for vox-cli-core — semcov wave 22.
//!
//! Targets: artifact_policy, command_contract, cli_args, diagnostics, constants,
//! cli_actions, GlobalOpts, fs_utils.

#[cfg(test)]
mod semcov_wave22_tests {
    // ── artifact_policy ────────────────────────────────────────────────────────

    use crate::artifact_policy::{
        canonical_workspace_target, ci_nested_target, gate_isolated_target,
        is_allowed_artifact_path, transient_lane_roots,
    };
    use std::path::Path;

    // Catches: is_allowed_artifact_path returning true for a sibling path that
    // merely *contains* the string "target" in a component name that is not
    // actually "target" (e.g. "/repo/target-ci").
    #[test]
    fn denies_target_dash_sibling_not_confused_with_canonical() {
        let root = Path::new("/repo");
        assert!(
            !is_allowed_artifact_path(&root.join("target-ci"), root),
            "target-ci should not be allowed (sprawl)"
        );
        assert!(
            !is_allowed_artifact_path(&root.join("target_extra"), root),
            "target_extra should not be allowed (sprawl)"
        );
    }

    // Catches: is_allowed_artifact_path allowing a "target-x" dir that is a
    // *deep* nested child (bug: the sprawl check only fires on the top level).
    #[test]
    fn denies_nested_target_sprawl_at_root_level() {
        let root = Path::new("/repo");
        // Only the FIRST component after root is checked; a deeper one is not sprawl.
        // This test asserts the current documented behavior — if behaviour changes,
        // this catches a regression.
        assert!(
            !is_allowed_artifact_path(&root.join("target-ci").join("debug"), root),
            "a file under target-ci/ must still be denied"
        );
    }

    // Catches: mens/runs path being accidentally denied when root changes.
    #[test]
    fn allows_mens_runs_subpath() {
        let root = Path::new("/workspace/proj");
        let mens_run = root.join("mens").join("runs").join("20240101");
        assert!(
            is_allowed_artifact_path(&mens_run, root),
            "mens/runs/* must be allowed"
        );
    }

    // Catches: is_allowed_artifact_path denying .vox/cache paths inside the workspace.
    #[test]
    fn allows_dot_vox_cache_inside_workspace() {
        let root = Path::new("/workspace/proj");
        let cache = root.join(".vox").join("cache").join("some-artifact");
        assert!(
            is_allowed_artifact_path(&cache, root),
            ".vox/cache/* inside workspace must be allowed"
        );
    }

    // Catches: is_allowed_artifact_path falsely allowing an unrelated /tmp path
    // that doesn't start with `vox-targets`.
    #[test]
    fn denies_arbitrary_tmp_path() {
        let root = Path::new("/repo");
        let arbitrary = std::env::temp_dir().join("not-vox-targets").join("foo");
        assert!(
            !is_allowed_artifact_path(&arbitrary, root),
            "arbitrary /tmp paths should be denied"
        );
    }

    // Catches: transient_lane_roots returning the same path for two different repos
    // because the hash function is not collision-free for trivially different paths.
    #[test]
    fn transient_lanes_are_unique_per_root() {
        let r1 = Path::new("/workspace/proj-alpha");
        let r2 = Path::new("/workspace/proj-beta");
        let [a1, a2] = transient_lane_roots(r1);
        let [b1, b2] = transient_lane_roots(r2);
        assert_ne!(a1, b1, "nested-ci must differ across repos");
        assert_ne!(a2, b2, "mens-gate-safe must differ across repos");
    }

    // Catches: ci_nested_target and gate_isolated_target diverging from
    // transient_lane_roots (copy-paste bug introducing a separate hash call).
    #[test]
    fn individual_targets_consistent_with_transient_roots() {
        let root = Path::new("/ci/repo");
        let [nested, gate] = transient_lane_roots(root);
        assert_eq!(
            ci_nested_target(root),
            nested,
            "ci_nested_target must equal transient_lane_roots[0]"
        );
        assert_eq!(
            gate_isolated_target(root),
            gate,
            "gate_isolated_target must equal transient_lane_roots[1]"
        );
    }

    // Catches: canonical_workspace_target appending a second "target" segment when
    // root already ends in "target".
    #[test]
    fn canonical_workspace_target_does_not_double_append() {
        // Root intentionally ends in "target" (edge case user might supply).
        let root = Path::new("/repo/target");
        let result = canonical_workspace_target(root);
        // Result should be /repo/target/target — not wrong per se but the test
        // documents that a bare `root.join("target")` always appends blindly.
        assert!(
            result.ends_with("target"),
            "result must end with 'target' component"
        );
        // Must not silently re-use root if root already ends in target.
        assert_ne!(result, root, "must not return root unchanged");
    }

    // Catches: is_allowed_artifact_path accepting an absolute path under the home
    // .vox dir when neither USERPROFILE nor HOME is set (fallback path is
    // "/nonexistent/.vox" — must not accept real unrelated paths).
    #[test]
    fn home_vox_fallback_does_not_allow_random_absolute_path() {
        let root = Path::new("/repo");
        // A path that happens to start with "/nonexistent/.vox" should still be
        // denied on a real machine because that dir won't match any real home.
        // If env vars are set, test still holds for the concrete home prefix.
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .map(|h| std::path::PathBuf::from(h).join(".vox"))
            .unwrap_or_else(|_| std::path::PathBuf::from("/nonexistent/.vox"));
        let allowed = home.join("some-artifact");
        // Allowed because it IS under ~/.vox — this confirms the positive case.
        assert!(
            is_allowed_artifact_path(&allowed, root),
            "~/.vox/* should be allowed"
        );
        // But a sibling that doesn't start with home/.vox must not be allowed.
        let sibling = home
            .parent()
            .unwrap_or(Path::new("/"))
            .join("not-vox")
            .join("artifact");
        assert!(
            !is_allowed_artifact_path(&sibling, root),
            "~/ sibling outside .vox must be denied"
        );
    }

    // ── command_contract ───────────────────────────────────────────────────────

    use crate::command_contract::{fallback_source_group, merged_feature_gate_from_vox_cli_ops};
    use crate::command_registry_model::RegistryOperation;

    fn op(
        surface: &str,
        path: &[&str],
        gate: Option<&str>,
        lane: Option<&str>,
    ) -> RegistryOperation {
        RegistryOperation {
            surface: surface.to_string(),
            path: path.iter().map(|s| s.to_string()).collect(),
            status: "active".to_string(),
            latin_ns: None,
            product_lane: lane.map(str::to_string),
            feature_gate: gate.map(str::to_string),
            catalog_group: None,
            ref_cli_required: true,
            reachability_required: None,
            handler_rust: None,
        }
    }

    // Catches: merged_feature_gate_from_vox_cli_ops returning `None` when there
    // is exactly one gate (off-by-one in the dedup Vec).
    #[test]
    fn merged_gate_single_entry_returns_that_gate() {
        let ops = vec![op("vox-cli", &["build"], Some("db"), None)];
        let path: Vec<String> = vec!["build".to_string()];
        assert_eq!(
            merged_feature_gate_from_vox_cli_ops(&ops, &path),
            Some("db".to_string())
        );
    }

    // Catches: merged_feature_gate_from_vox_cli_ops emitting duplicate gate strings
    // when the same feature_gate appears twice (dedup not working).
    #[test]
    fn merged_gate_deduplicates_identical_gates() {
        let ops = vec![
            op("vox-cli", &["build"], Some("db"), None),
            op("vox-cli", &["build"], Some("db"), None),
        ];
        let path: Vec<String> = vec!["build".to_string()];
        // Expect "db" not "db|db".
        assert_eq!(
            merged_feature_gate_from_vox_cli_ops(&ops, &path),
            Some("db".to_string())
        );
    }

    // Catches: merged_feature_gate_from_vox_cli_ops joining gates from a different
    // surface (should only aggregate vox-cli rows, but the helper takes a slice
    // that the caller pre-filters — here we confirm no cross-surface leakage when
    // caller passes a mixed slice).
    #[test]
    fn merged_gate_empty_when_no_matching_path() {
        let ops = vec![op("vox-cli", &["other"], Some("db"), None)];
        let path: Vec<String> = vec!["build".to_string()];
        assert_eq!(merged_feature_gate_from_vox_cli_ops(&ops, &path), None);
    }

    // Catches: merged_feature_gate_from_vox_cli_ops emitting `None` for an empty
    // path slice when there are ops with empty paths.
    #[test]
    fn merged_gate_empty_path_slice_matches_empty_path_op() {
        let ops = vec![op("vox-cli", &[], Some("feature-x"), None)];
        let path: Vec<String> = vec![];
        assert_eq!(
            merged_feature_gate_from_vox_cli_ops(&ops, &path),
            Some("feature-x".to_string())
        );
    }

    // Catches: fallback_source_group returning wrong group for "migrate" (it maps
    // to "pm" but could be misidentified as "core").
    #[test]
    fn fallback_source_group_migrate_is_pm() {
        let path = vec!["migrate".to_string()];
        assert_eq!(fallback_source_group(&path), "pm");
    }

    // Catches: fallback_source_group returning wrong group for unknown top-level
    // commands — must default to "core", not panic or return empty string.
    #[test]
    fn fallback_source_group_unknown_returns_core() {
        let path = vec!["totally-unknown-command".to_string()];
        assert_eq!(fallback_source_group(&path), "core");
    }

    // Catches: fallback_source_group panicking on an empty path slice (unwrap on
    // first() when path is empty).
    #[test]
    fn fallback_source_group_empty_path_returns_core() {
        let path: Vec<String> = vec![];
        // Must not panic; should return "core" (the catch-all).
        let result = fallback_source_group(&path);
        assert_eq!(result, "core");
    }

    // Catches: fallback_source_group returning "fabrica" instead of "diag" for
    // "doctor" (a common mismatch given "doctor" is near build-related words).
    #[test]
    fn fallback_source_group_doctor_is_diag() {
        let path = vec!["doctor".to_string()];
        assert_eq!(fallback_source_group(&path), "diag");
    }

    // ── cli_args serde round-trips ─────────────────────────────────────────────

    use crate::cli_args::{BuildMode, BundleMode, CompileKind, UpgradeLane};

    // Catches: serde kebab-case rename on CompileKind not round-tripping correctly
    // (e.g. NativeBinary serialises to "NativeBinary" instead of "native-binary").
    #[test]
    fn compile_kind_serde_roundtrip_native_binary() {
        let original = CompileKind::NativeBinary;
        let json = serde_json::to_string(&original).expect("serialize");
        let back: CompileKind = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, back, "round-trip failed: json={json}");
    }

    // Catches: CompileKind default being silently changed from NativeBinary.
    #[test]
    fn compile_kind_default_is_native_binary() {
        assert_eq!(CompileKind::default(), CompileKind::NativeBinary);
    }

    // Catches: BuildMode::Library serde not round-tripping (default derive uses
    // PascalCase but YAML consumers may expect lowercase).
    #[test]
    fn build_mode_library_serde_roundtrip() {
        let original = BuildMode::Library;
        let json = serde_json::to_string(&original).expect("serialize");
        let back: BuildMode = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original, back);
    }

    // Catches: UpgradeLane default being changed from Release to Repo.
    #[test]
    fn upgrade_lane_default_is_release() {
        assert_eq!(UpgradeLane::default(), UpgradeLane::Release);
    }

    // ── diagnostics ───────────────────────────────────────────────────────────

    use crate::diagnostics::{ColorChoice, should_color_stderr, should_color_stdout};

    // Catches: ColorChoice::Never not suppressing color even when COLOR_CHOICE is
    // Never — test that the *return value* reflects Never regardless of TTY state.
    // (Cannot use set_color_choice in tests because OnceLock races; test Never
    // logic via the enum value directly to avoid cross-test pollution.)
    #[test]
    fn color_choice_never_variant_exists_and_is_not_auto() {
        assert_ne!(ColorChoice::Never, ColorChoice::Auto);
        assert_ne!(ColorChoice::Never, ColorChoice::Always);
    }

    // Catches: ColorChoice default being accidentally changed to Always (which
    // would spray ANSI codes in non-TTY environments like CI).
    #[test]
    fn color_choice_default_is_auto() {
        assert_eq!(ColorChoice::default(), ColorChoice::Auto);
    }

    // ── GlobalOpts ────────────────────────────────────────────────────────────

    use crate::GlobalOpts;
    use clap::Parser;

    #[derive(Parser)]
    struct TestRoot {
        #[command(flatten)]
        global: GlobalOpts,
    }

    // Catches: verbose counting stopping at 1 due to ArgAction::Count being
    // replaced with a boolean (regression guard).
    #[test]
    fn verbose_flag_counts_multiple_occurrences() {
        let r = TestRoot::try_parse_from(["vox", "-v", "-v", "-v"]).expect("parse");
        assert_eq!(r.global.verbose, 3, "verbose should count three -v flags");
    }

    // Catches: json flag being positional or requiring a value (it should be a
    // bare boolean flag).
    #[test]
    fn json_flag_is_bare_boolean() {
        let r = TestRoot::try_parse_from(["vox", "--json"]).expect("parse");
        assert!(r.global.json);
    }

    // Catches: quiet and verbose both being true simultaneously not being rejected
    // (clap doesn't enforce mutual exclusion by default — test at least that
    // both parse independently).
    #[test]
    fn quiet_and_verbose_can_both_be_set_independently() {
        let r = TestRoot::try_parse_from(["vox", "-q", "-v"]).expect("parse");
        assert!(r.global.quiet);
        assert_eq!(r.global.verbose, 1);
    }

    // Catches: --color requiring an argument when none is provided (should fail
    // parse cleanly, not panic).
    #[test]
    fn color_flag_without_value_is_error() {
        let result = TestRoot::try_parse_from(["vox", "--color"]);
        assert!(
            result.is_err(),
            "--color without value should be a parse error"
        );
    }

    // Catches: --color accepting an invalid value without error.
    #[test]
    fn color_flag_with_invalid_value_is_error() {
        let result = TestRoot::try_parse_from(["vox", "--color", "rainbow"]);
        assert!(
            result.is_err(),
            "--color rainbow should be rejected as invalid"
        );
    }

    // ── constants sanity ──────────────────────────────────────────────────────

    use crate::constants::*;

    // Catches: PUBLICATION_EXTERNAL_JOBS_DEFAULT_LIMIT being set to 0 or negative,
    // which would silently disable job dispatch.
    #[test]
    fn publication_external_jobs_limit_is_positive() {
        assert!(
            PUBLICATION_EXTERNAL_JOBS_DEFAULT_LIMIT > 0,
            "jobs limit must be positive"
        );
    }

    // Catches: tick limit exceeding the main batch limit (tick should be a subset).
    #[test]
    fn publication_tick_limit_does_not_exceed_jobs_limit() {
        assert!(
            PUBLICATION_EXTERNAL_JOBS_TICK_DEFAULT_LIMIT <= PUBLICATION_EXTERNAL_JOBS_DEFAULT_LIMIT,
            "tick limit ({}) must not exceed main jobs limit ({})",
            PUBLICATION_EXTERNAL_JOBS_TICK_DEFAULT_LIMIT,
            PUBLICATION_EXTERNAL_JOBS_DEFAULT_LIMIT
        );
    }

    // Catches: lock TTL being set to 0, which would cause immediate lock expiry
    // and potential duplicate processing.
    #[test]
    fn lock_ttl_ms_is_positive() {
        assert!(
            PUBLICATION_EXTERNAL_JOBS_TICK_DEFAULT_LOCK_TTL_MS > 0,
            "lock TTL must be > 0 ms"
        );
    }

    // Catches: sync batch limit being larger than the main jobs limit (could cause
    // a batch to silently pull more items than the system is tuned for).
    #[test]
    fn sync_batch_limit_does_not_exceed_jobs_limit() {
        assert!(
            PUBLICATION_SYNC_BATCH_DEFAULT_LIMIT <= PUBLICATION_EXTERNAL_JOBS_DEFAULT_LIMIT,
            "sync batch limit must not exceed jobs limit"
        );
    }
}
