//! P0-T4: SecretBag — gated injection of JWE-decrypted secrets into task env.

use vox_orchestrator::a2a::secret_bag::SecretBag;

#[test]
fn bag_only_exposes_declared_secrets() {
    let bag = SecretBag::from_decrypted(serde_json::json!({
        "VoxGitHubToken": "ghp_AAA",
        "VoxOpenAiKey":   "sk-XYZ",
    }))
    .unwrap();

    let env = bag.env_for_declared(&["VoxGitHubToken".to_string()]);
    assert_eq!(env.len(), 1);
    // Naive CamelCase → SCREAMING_SNAKE: VoxGitHubToken → VOX_GIT_HUB_TOKEN
    assert_eq!(env[0].0, "VOX_GIT_HUB_TOKEN");
    assert_eq!(env[0].1, "ghp_AAA");
}

#[test]
fn bag_skips_unknown_declarations() {
    let bag = SecretBag::from_decrypted(serde_json::json!({
        "VoxGitHubToken": "ghp_AAA",
    }))
    .unwrap();
    let env = bag.env_for_declared(&[
        "VoxGitHubToken".to_string(),
        "VoxOpenAiKey".to_string(), // not in the bag
    ]);
    assert_eq!(env.len(), 1);
    assert_eq!(env[0].0, "VOX_GIT_HUB_TOKEN");
}

#[test]
fn bag_redacts_in_debug_format() {
    let bag = SecretBag::from_decrypted(serde_json::json!({
        "VoxGitHubToken": "ghp_AAA",
    }))
    .unwrap();
    let dbg = format!("{bag:?}");
    assert!(!dbg.contains("ghp_AAA"));
    assert!(dbg.contains("[redacted") || dbg.contains("len="));
}
