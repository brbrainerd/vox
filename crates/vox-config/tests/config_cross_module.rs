//! Cross-module integration: defaults compose paths, rollout snapshot, and routing helpers.

use vox_config::{
    InferenceProfile, VoxConfig,
    rollout::rollout_flag_snapshot,
    routing_policy::{AutoRoutingPriority, resolve_openrouter_model},
};

#[test]
fn vox_config_default_wires_hitl_paths_and_training_defaults() {
    let cfg = VoxConfig::default();
    assert!(cfg.hitl.enabled);
    assert!(cfg.gamify_enabled);
    assert_eq!(cfg.train_batch_size, 256);
    assert!(
        cfg.model_dir.ends_with("models"),
        "unexpected model_dir: {:?}",
        cfg.model_dir
    );
}

#[test]
fn inference_profile_default_aligns_with_rollout_snapshot_shape() {
    assert_eq!(InferenceProfile::default(), InferenceProfile::DesktopOllama);
    let snap = rollout_flag_snapshot();
    let json = serde_json::to_string(&snap).expect("RolloutFlagSnapshot serializes");
    assert!(json.contains("orchestration_lineage_persist"));
}

#[test]
fn auto_routing_priority_default_used_with_openrouter_resolution() {
    let priority = AutoRoutingPriority::default();
    assert_eq!(priority.efficiency, 25);
    let model = resolve_openrouter_model(Some("anthropic/claude-3-haiku".into()));
    assert!(!model.trim().is_empty());
}

#[test]
fn vox_config_default_serializes_and_deserializes_via_json() {
    let cfg = VoxConfig::default();
    let json = serde_json::to_string(&cfg).expect("VoxConfig should serialize to JSON");
    let restored: VoxConfig = serde_json::from_str(&json).expect("VoxConfig should deserialize");
    assert_eq!(restored.model, cfg.model);
    assert_eq!(restored.train_batch_size, cfg.train_batch_size);
    assert_eq!(restored.hitl.enabled, cfg.hitl.enabled);
    assert_eq!(restored.gamify_mode, cfg.gamify_mode);
}
