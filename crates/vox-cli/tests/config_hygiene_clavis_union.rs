/// Phase 3 — Clavis-managed secret env names are folded into the Check D
/// "recognized" set so that credentials like `GEMINI_API_KEY` and `VAULT_TOKEN`
/// don't need manual rows in `contracts/config/registry.v1.yaml`.
///
/// This test calls `build_recognized_env_vars` (the production helper used by
/// `run()`) and asserts that Clavis-managed names are present without YAML rows.

#[test]
fn clavis_secret_names_are_recognized_without_yaml_row() {
    // Empty YAML — no manual rows at all.
    let recognized = vox_cli::commands::ci::config_hygiene::build_recognized_env_vars("");

    // GEMINI_API_KEY and OPENROUTER_API_KEY are in managed_secret_env_names()
    assert!(
        recognized.contains("GEMINI_API_KEY"),
        "GEMINI_API_KEY should be auto-recognized via Clavis spec"
    );
    assert!(
        recognized.contains("OPENROUTER_API_KEY"),
        "OPENROUTER_API_KEY should be auto-recognized via Clavis spec"
    );

    // A purely fictitious key is NOT in the set.
    assert!(
        !recognized.contains("ZZ_FAKE_KEY_NEVER_REAL"),
        "ZZ_FAKE_KEY_NEVER_REAL must NOT be in the recognized set"
    );
}

#[test]
fn clavis_union_does_not_remove_existing_yaml_rows() {
    let yaml = "env_var: VOX_WASM_SKILL_FUEL\nenv_var: VOX_MENS_DEFAULT_MODEL";
    let recognized = vox_cli::commands::ci::config_hygiene::build_recognized_env_vars(yaml);

    // Manually-registered VOX_* names must still be present.
    assert!(recognized.contains("VOX_WASM_SKILL_FUEL"));
    assert!(recognized.contains("VOX_MENS_DEFAULT_MODEL"));
    // And Clavis names too.
    assert!(recognized.contains("GEMINI_API_KEY"));
}
