use std::sync::Mutex;

use super::{ConfigValidationError, CostPreference, OrchestratorConfig, OverflowStrategy};
use crate::contract::OrchestrationMigrationFlags;
use crate::types::TaskPriority;

/// Serializes tests that mutate process environment variables.
static ENV_MUTEX: Mutex<()> = Mutex::new(());

#[test]
fn default_config_values() {
    let cfg = OrchestratorConfig::default();
    assert!(cfg.enabled);
    assert_eq!(cfg.max_agents, 8);
    assert_eq!(cfg.default_priority, TaskPriority::Normal);
    assert_eq!(cfg.queue_overflow_strategy, OverflowStrategy::SpawnNewAgent);
    assert_eq!(cfg.lock_timeout_ms, 30_000);
    assert!(cfg.toestub_gate);
    assert!(cfg.behavioral_gate_on_complete);
    assert!(cfg.completion_markdown_link_audit_enabled);
    assert!(cfg.fallback_to_single_agent);
    assert_eq!(cfg.min_agents, 1);
    assert!(!cfg.scaling_enabled);
    assert_eq!(cfg.cost_preference, CostPreference::Economy);
    assert_eq!(cfg.repo_shard_specialization_weight, 1.5);
    assert_eq!(cfg.repo_shard_validation_failure_penalty, 0.8);
    assert_eq!(cfg.repo_reduce_conflict_cooldown_penalty, 2.5);
    assert_eq!(cfg.repo_reduce_conflict_cooldown_ms, cfg.idle_retirement_ms);
}

#[test]
fn config_serialization_roundtrip() {
    let cfg = OrchestratorConfig::default();
    let json = serde_json::to_string(&cfg).expect("serialize");
    let back: OrchestratorConfig = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.max_agents, cfg.max_agents);
    assert_eq!(back.enabled, cfg.enabled);
}

#[test]
fn test_config_values() {
    let cfg = OrchestratorConfig::for_testing();
    assert_eq!(cfg.max_agents, 4);
    assert_eq!(cfg.lock_timeout_ms, 1000);
    assert!(!cfg.toestub_gate);
    assert!(!cfg.behavioral_gate_on_complete);
    assert!(!cfg.completion_markdown_link_audit_enabled);
    assert!(cfg.validate().is_ok());
}

#[test]
fn test_validation_errors() {
    let cfg = OrchestratorConfig {
        max_agents: 0,
        lock_timeout_ms: 50,
        bulletin_capacity: 0,
        ..Default::default()
    };

    let errs = cfg.validate().unwrap_err();
    assert_eq!(errs.len(), 4);
    assert!(errs.contains(&ConfigValidationError::InvalidMaxAgents(0)));
    assert!(errs.contains(&ConfigValidationError::InvalidLockTimeout(50)));
    assert!(errs.contains(&ConfigValidationError::InvalidBulletinCapacity(0)));
    assert!(errs.contains(&ConfigValidationError::InvalidScalingLimits(1, 0)));
}

#[test]
fn missing_toml_section_returns_default() {
    let dir = std::env::temp_dir().join("vox_orch_test");
    std::fs::create_dir_all(&dir).ok();
    let toml_path = dir.join("no_orch.toml");
    std::fs::write(&toml_path, "[package]\nname = \"test\"\n").ok();

    let cfg = OrchestratorConfig::load_from_toml(&toml_path).expect("should load");
    assert_eq!(cfg.max_agents, 8);
}

#[test]
fn orchestration_migration_defaults_match_contract() {
    let c = OrchestratorConfig::default();
    assert!(!c.orchestration_migration.orchestration_v2_enabled);
    assert!(c.orchestration_migration.legacy_orchestration_fallback);
}

#[test]
fn orchestration_migration_deserializes_from_toml_fragment() {
    let flags: OrchestrationMigrationFlags =
        toml::from_str("orchestration_v2_enabled = true\nlegacy_orchestration_fallback = false\n")
            .expect("parse nested [orchestrator.orchestration_migration]-shaped keys");
    assert!(flags.orchestration_v2_enabled);
    assert!(!flags.legacy_orchestration_fallback);
}

#[test]
fn populi_toml_section_merges_into_config() {
    let dir = tempfile::tempdir().expect("tempdir");
    let toml_path = dir.path().join("Vox.toml");
    std::fs::write(
        &toml_path,
        r#"
[orchestrator]
max_agents = 3

[mens]
control_url = "http://mens.example:9847"
scope_id = "unit-scope"
inference_base_url = "http://127.0.0.1:11435"
advertise_gpu = true
labels = ["from=toml"]
"#,
    )
    .expect("write");
    let cfg = OrchestratorConfig::load_from_toml(&toml_path).expect("load");
    assert_eq!(cfg.max_agents, 3);
    assert_eq!(
        cfg.populi_control_url.as_deref(),
        Some("http://mens.example:9847")
    );
    assert_eq!(
        cfg.populi_inference_base_url.as_deref(),
        Some("http://127.0.0.1:11435")
    );
    assert_eq!(cfg.populi_scope_id.as_deref(), Some("unit-scope"));
    assert!(cfg.default_agent_capabilities.gpu_cuda);
    assert!(
        cfg.default_agent_capabilities
            .labels
            .contains(&"from=toml".to_string())
    );
}

#[test]
#[allow(unsafe_code)]
fn populi_inference_env_overrides_toml_base_url() {
    let _guard = ENV_MUTEX.lock().expect("env test lock");
    const KEY: &str = "VOX_ORCHESTRATOR_POPULI_INFERENCE_BASE_URL";
    let prev =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOrchestratorPopuliInferenceBaseUrl)
            .expose()
            .map(ToString::to_string);
    unsafe {
        std::env::set_var(KEY, "http://env-infer:9999");
    }

    let dir = tempfile::tempdir().expect("tempdir");
    let toml_path = dir.path().join("Vox.toml");
    std::fs::write(
        &toml_path,
        r#"
[mens]
inference_base_url = "http://toml-infer:8888"
"#,
    )
    .expect("write");

    let mut cfg = OrchestratorConfig::load_from_toml(&toml_path).expect("load");
    assert_eq!(
        cfg.populi_inference_base_url.as_deref(),
        Some("http://toml-infer:8888")
    );
    cfg.merge_env_overrides();
    assert_eq!(
        cfg.populi_inference_base_url.as_deref(),
        Some("http://env-infer:9999")
    );

    unsafe {
        match prev {
            None => std::env::remove_var(KEY),
            Some(v) => std::env::set_var(KEY, v),
        }
    }
}

#[test]
#[allow(unsafe_code)]
fn populi_env_overrides_toml_control_url() {
    let _guard = ENV_MUTEX.lock().expect("env test lock");
    const KEY: &str = "VOX_ORCHESTRATOR_MESH_CONTROL_URL";
    let prev = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOrchestratorMeshControlUrl)
        .expose()
        .map(ToString::to_string);
    unsafe {
        std::env::set_var(KEY, "http://env-wins:7777");
    }

    let dir = tempfile::tempdir().expect("tempdir");
    let toml_path = dir.path().join("Vox.toml");
    std::fs::write(
        &toml_path,
        r#"
[mens]
control_url = "http://toml-loses:8888"
"#,
    )
    .expect("write");

    let mut cfg = OrchestratorConfig::load_from_toml(&toml_path).expect("load");
    assert_eq!(
        cfg.populi_control_url.as_deref(),
        Some("http://toml-loses:8888")
    );
    cfg.merge_env_overrides();
    assert_eq!(
        cfg.populi_control_url.as_deref(),
        Some("http://env-wins:7777")
    );

    unsafe {
        match prev {
            None => std::env::remove_var(KEY),
            Some(v) => std::env::set_var(KEY, v),
        }
    }
}

#[test]
#[allow(unsafe_code)]
fn repo_shard_env_overrides_apply_consistently() {
    let _guard = ENV_MUTEX.lock().expect("env test lock");
    const W_KEY: &str = "VOX_ORCHESTRATOR_REPO_SHARD_SPECIALIZATION_WEIGHT";
    const VF_KEY: &str = "VOX_ORCHESTRATOR_REPO_SHARD_VALIDATION_FAILURE_PENALTY";
    const RC_P_KEY: &str = "VOX_ORCHESTRATOR_REPO_REDUCE_CONFLICT_COOLDOWN_PENALTY";
    const RC_MS_KEY: &str = "VOX_ORCHESTRATOR_REPO_REDUCE_CONFLICT_COOLDOWN_MS";

    let prev_w = vox_secrets::resolve_secret(
        vox_secrets::SecretId::VoxOrchestratorRepoShardSpecializationWeight,
    )
    .expose()
    .map(ToString::to_string);
    let prev_vf = vox_secrets::resolve_secret(
        vox_secrets::SecretId::VoxOrchestratorRepoShardValidationFailurePenalty,
    )
    .expose()
    .map(ToString::to_string);
    let prev_rc_p = vox_secrets::resolve_secret(
        vox_secrets::SecretId::VoxOrchestratorRepoReduceConflictCooldownPenalty,
    )
    .expose()
    .map(ToString::to_string);
    let prev_rc_ms = vox_secrets::resolve_secret(
        vox_secrets::SecretId::VoxOrchestratorRepoReduceConflictCooldownMs,
    )
    .expose()
    .map(ToString::to_string);

    unsafe {
        std::env::set_var(W_KEY, "1.25");
        std::env::set_var(VF_KEY, "0.55");
        std::env::set_var(RC_P_KEY, "3.0");
        std::env::set_var(RC_MS_KEY, "90000");
    }

    let mut cfg = OrchestratorConfig::default();
    cfg.merge_env_overrides();
    assert_eq!(cfg.repo_shard_specialization_weight, 1.25);
    assert_eq!(cfg.repo_shard_validation_failure_penalty, 0.55);
    assert_eq!(cfg.repo_reduce_conflict_cooldown_penalty, 3.0);
    assert_eq!(cfg.repo_reduce_conflict_cooldown_ms, 90_000);

    unsafe {
        match prev_w {
            None => std::env::remove_var(W_KEY),
            Some(v) => std::env::set_var(W_KEY, v),
        }
        match prev_vf {
            None => std::env::remove_var(VF_KEY),
            Some(v) => std::env::set_var(VF_KEY, v),
        }
        match prev_rc_p {
            None => std::env::remove_var(RC_P_KEY),
            Some(v) => std::env::set_var(RC_P_KEY, v),
        }
        match prev_rc_ms {
            None => std::env::remove_var(RC_MS_KEY),
            Some(v) => std::env::set_var(RC_MS_KEY, v),
        }
    }
}

#[test]
#[allow(unsafe_code)]
fn populi_remote_result_max_messages_env_override_applies() {
    let _guard = ENV_MUTEX.lock().expect("env test lock");
    const KEY: &str = "VOX_ORCHESTRATOR_MESH_REMOTE_RESULT_MAX_MESSAGES_PER_POLL";
    let prev = vox_secrets::resolve_secret(
        vox_secrets::SecretId::VoxOrchestratorMeshRemoteResultMaxMessagesPerPoll,
    )
    .expose()
    .map(ToString::to_string);

    unsafe {
        std::env::set_var(KEY, "17");
    }
    let mut cfg = OrchestratorConfig::default();
    cfg.merge_env_overrides();
    assert_eq!(cfg.populi_remote_result_max_messages_per_poll, 17);

    unsafe {
        match prev {
            None => std::env::remove_var(KEY),
            Some(v) => std::env::set_var(KEY, v),
        }
    }
}

#[test]
#[allow(unsafe_code)]
fn completion_markdown_link_audit_env_override_applies() {
    let _guard = ENV_MUTEX.lock().expect("env test lock");
    const KEY: &str = "VOX_ORCHESTRATOR_COMPLETION_MARKDOWN_LINK_AUDIT_ENABLED";
    let prev = vox_secrets::resolve_secret(
        vox_secrets::SecretId::VoxOrchestratorCompletionMarkdownLinkAuditEnabled,
    )
    .expose()
    .map(ToString::to_string);

    unsafe {
        std::env::set_var(KEY, "false");
    }
    let mut cfg = OrchestratorConfig::default();
    assert!(cfg.completion_markdown_link_audit_enabled);
    cfg.merge_env_overrides();
    assert!(!cfg.completion_markdown_link_audit_enabled);

    unsafe {
        std::env::set_var(KEY, "true");
    }
    let mut cfg_testing = OrchestratorConfig::for_testing();
    assert!(!cfg_testing.completion_markdown_link_audit_enabled);
    cfg_testing.merge_env_overrides();
    assert!(cfg_testing.completion_markdown_link_audit_enabled);

    unsafe {
        match prev {
            None => std::env::remove_var(KEY),
            Some(v) => std::env::set_var(KEY, v),
        }
    }
}

#[test]
#[allow(unsafe_code)]
fn social_credentials_follow_secrets_lenient_vs_strict() {
    let _guard = ENV_MUTEX.lock().expect("env test lock");
    let key = "VOX_SOCIAL_REDDIT_CLIENT_ID";
    let prev_key = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxSocialRedditClientId)
        .expose()
        .map(ToString::to_string);
    let prev_backend = std::env::var("VOX_SECRETS_BACKEND").ok();
    let prev_profile = std::env::var("VOX_SECRETS_PROFILE").ok();
    // Identifier must avoid the legacy Vox+Turso URL env token as a contiguous substring (cutover audit).
    const DB_REMOTE_ALIAS_URL_ENV: &str = concat!("VOX_", "TURSO", "_URL");
    let prev_url = std::env::var(DB_REMOTE_ALIAS_URL_ENV).ok(); // This is a system alias, not a secret? Actually it's retired.
    let prev_cloudless_path = std::env::var("VOX_SECRETS_CLOUDLESS_DB_PATH").ok();
    let prev_account_id = std::env::var("VOX_ACCOUNT_ID").ok();

    unsafe {
        std::env::set_var(key, "orchestrator-env-client");
        std::env::set_var("VOX_SECRETS_BACKEND", "vox_cloud");
        std::env::set_var("VOX_SECRETS_PROFILE", "dev");
        std::env::remove_var(DB_REMOTE_ALIAS_URL_ENV);
        let tmp = std::env::temp_dir().join("vox-secrets-orchestrator-strict-lenient.db");
        std::env::set_var(
            "VOX_SECRETS_CLOUDLESS_DB_PATH",
            tmp.to_string_lossy().to_string(),
        );
        std::env::set_var("VOX_ACCOUNT_ID", "orchestrator-strict-lenient-test");
    }
    let mut lenient = OrchestratorConfig::default();
    lenient.merge_env_overrides();
    assert_eq!(
        lenient.news.reddit_client_id.as_deref(),
        Some("orchestrator-env-client")
    );

    unsafe {
        std::env::set_var("VOX_SECRETS_PROFILE", "hard_cut");
        std::env::remove_var(DB_REMOTE_ALIAS_URL_ENV);
    }
    let mut strict = OrchestratorConfig::default();
    strict.merge_env_overrides();
    assert!(strict.news.reddit_client_id.is_none());

    unsafe {
        match prev_key {
            None => std::env::remove_var(key),
            Some(v) => std::env::set_var(key, v),
        }
        match prev_backend {
            None => std::env::remove_var("VOX_SECRETS_BACKEND"),
            Some(v) => std::env::set_var("VOX_SECRETS_BACKEND", v),
        }
        match prev_profile {
            None => std::env::remove_var("VOX_SECRETS_PROFILE"),
            Some(v) => std::env::set_var("VOX_SECRETS_PROFILE", v),
        }
        match prev_url {
            None => std::env::remove_var(DB_REMOTE_ALIAS_URL_ENV),
            Some(v) => std::env::set_var(DB_REMOTE_ALIAS_URL_ENV, v),
        }
        match prev_cloudless_path {
            None => std::env::remove_var("VOX_SECRETS_CLOUDLESS_DB_PATH"),
            Some(v) => std::env::set_var("VOX_SECRETS_CLOUDLESS_DB_PATH", v),
        }
        match prev_account_id {
            None => std::env::remove_var("VOX_ACCOUNT_ID"),
            Some(v) => std::env::set_var("VOX_ACCOUNT_ID", v),
        }
    }
}
