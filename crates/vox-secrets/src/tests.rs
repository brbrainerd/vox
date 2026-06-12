use std::sync::Mutex;

use crate::backend::{NoopBackend, UnavailableBackend};
use crate::resolver::{ResolveOptions, ResolveProfile, SecretResolver};
use crate::spec::{
    Profile, RequirementMode, RequirementSet, SecretBundle, SecretClass, SecretId, Workflow,
    required_for_profile, requirements_for_bundle, requirements_for_profile_mode,
};
use crate::{ResolutionStatus, resolve_env_only};
use std::sync::atomic::{AtomicUsize, Ordering};

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
#[allow(unsafe_code)]
fn canonical_env_wins_over_alias() {
    let _g = ENV_LOCK.lock().expect("env lock");
    unsafe {
        std::env::set_var("GEMINI_API_KEY", "canonical");
        std::env::set_var("GOOGLE_AI_STUDIO_KEY", "alias");
    }
    let resolved = resolve_env_only(SecretId::GeminiApiKey);
    assert_eq!(resolved.expose(), Some("canonical"));
    assert!(matches!(resolved.status, ResolutionStatus::Present));
    unsafe {
        std::env::remove_var("GEMINI_API_KEY");
        std::env::remove_var("GOOGLE_AI_STUDIO_KEY");
    }
}

#[test]
#[allow(unsafe_code)]
fn backend_unavailable_status_is_explicit() {
    let _g = ENV_LOCK.lock().expect("env lock");
    unsafe {
        std::env::remove_var("OPENROUTER_API_KEY");
    }
    let resolver = SecretResolver::new(UnavailableBackend {
        reason: "feature disabled".to_string(),
    });
    let resolved = resolver.resolve(
        SecretId::OpenRouterApiKey,
        &ResolveOptions {
            include_env: true,
            include_auth_json: false,
            include_populi_env: false,
            profile: ResolveProfile::DevLenient,
            caller_context: "test".to_string(),
        },
    );
    assert!(matches!(
        resolved.status,
        ResolutionStatus::BackendUnavailable
    ));
    assert!(
        resolved
            .detail
            .unwrap_or_default()
            .contains("feature disabled")
    );
}

#[test]
#[allow(unsafe_code)]
fn env_only_ignores_backend() {
    let _g = ENV_LOCK.lock().expect("env lock");
    unsafe {
        std::env::remove_var("OPENROUTER_API_KEY");
    }
    let resolved = SecretResolver::new(NoopBackend).resolve(
        SecretId::OpenRouterApiKey,
        &ResolveOptions {
            include_env: true,
            include_auth_json: false,
            include_populi_env: false,
            profile: ResolveProfile::DevLenient,
            caller_context: "test".to_string(),
        },
    );
    assert!(matches!(resolved.status, ResolutionStatus::MissingRequired));
}

#[test]
fn profile_requirements_are_dynamic() {
    let dev = required_for_profile(Workflow::Chat, Profile::Dev);
    let ci = required_for_profile(Workflow::Chat, Profile::Ci);
    assert!(!dev.contains(&SecretId::OpenRouterApiKey));
    assert!(ci.contains(&SecretId::ForgeToken));
}

#[test]
fn openrouter_visible_in_local_chat_optionals_but_not_blocking_in_dev() {
    // `vox secrets status` (Local/Dev) must surface OpenRouter like sibling
    // providers — it was silently omitted because it lived only in the Cloud
    // blocking set, not in the optional/observable set.
    let req = requirements_for_profile_mode(Workflow::Chat, Profile::Dev, RequirementMode::Local);
    assert!(
        req.optional.contains(&SecretId::OpenRouterApiKey),
        "OpenRouter must be an observable optional in Local mode"
    );
    // Invariant preserved: still non-blocking in Dev (the primary-cloud credential
    // is only blocking under Cloud mode).
    assert!(
        req.blocking.is_empty(),
        "Local Chat must have no blocking requirements"
    );
    let dev = required_for_profile(Workflow::Chat, Profile::Dev);
    assert!(!dev.contains(&SecretId::OpenRouterApiKey));
}

#[test]
fn workflow_requirements_have_any_of_for_chat() {
    let req = requirements_for_profile_mode(Workflow::Chat, Profile::Dev, RequirementMode::Cloud);
    assert!(
        req.blocking
            .iter()
            .any(|group| matches!(group, RequirementSet::AllOf(_)))
    );
}

#[test]
fn bundle_requirements_are_defined() {
    let local = requirements_for_bundle(SecretBundle::MinimalLocalDev);
    let cloud = requirements_for_bundle(SecretBundle::MinimalCloudDev);
    assert!(local.blocking.is_empty());
    assert!(!cloud.blocking.is_empty());
}

#[test]
#[allow(unsafe_code)]
fn deprecated_alias_marks_status() {
    let _g = ENV_LOCK.lock().expect("env lock");
    unsafe {
        std::env::set_var("GOOGLE_AI_STUDIO_KEY", "legacy");
        std::env::remove_var("GEMINI_API_KEY");
    }
    let resolved = resolve_env_only(SecretId::GeminiApiKey);
    assert!(matches!(
        resolved.status,
        ResolutionStatus::DeprecatedAliasUsed
    ));
    unsafe {
        std::env::remove_var("GOOGLE_AI_STUDIO_KEY");
    }
}

#[test]
#[allow(unsafe_code)]
fn strict_profile_rejects_deprecated_alias() {
    let _g = ENV_LOCK.lock().expect("env lock");
    unsafe {
        std::env::set_var("GOOGLE_AI_STUDIO_KEY", "legacy");
        std::env::remove_var("GEMINI_API_KEY");
    }
    let resolver = SecretResolver::new(NoopBackend);
    let resolved = resolver.resolve(
        SecretId::GeminiApiKey,
        &ResolveOptions {
            include_env: true,
            include_auth_json: false,
            include_populi_env: false,
            profile: ResolveProfile::HardCutStrict,
            caller_context: "test".to_string(),
        },
    );
    assert!(matches!(
        resolved.status,
        ResolutionStatus::RejectedLegacyAlias
    ));
    unsafe {
        std::env::remove_var("GOOGLE_AI_STUDIO_KEY");
    }
}

#[test]
#[allow(unsafe_code)]
fn strict_profile_rejects_transport_env_source() {
    let _g = ENV_LOCK.lock().expect("env lock");
    unsafe {
        std::env::set_var("VOX_WEBHOOK_SIGNING_SECRET", "super-secret");
    }
    let resolver = SecretResolver::new(NoopBackend);
    let resolved = resolver.resolve(
        SecretId::WebhookSigningSecret,
        &ResolveOptions {
            include_env: true,
            include_auth_json: false,
            include_populi_env: false,
            profile: ResolveProfile::ProdStrict,
            caller_context: "test".to_string(),
        },
    );
    assert!(matches!(
        resolved.status,
        ResolutionStatus::RejectedSourcePolicy
    ));
    unsafe {
        std::env::remove_var("VOX_WEBHOOK_SIGNING_SECRET");
    }
}

#[test]
fn secret_metadata_is_defined_for_all_specs() {
    for spec in crate::all_specs() {
        let metadata = spec.id.metadata();
        // Account secrets should be persistable unless explicitly local-only.
        if matches!(metadata.class, SecretClass::Account) {
            assert!(metadata.persistable_account_secret || metadata.device_local_only);
        }
    }
}

#[test]
#[allow(unsafe_code)]
fn strict_cloudless_can_disable_env_plaintext_fallback() {
    let _g = ENV_LOCK.lock().expect("env lock");
    unsafe {
        std::env::set_var("OPENROUTER_API_KEY", "plaintext-env-secret");
    }
    let resolver = SecretResolver::new(NoopBackend);
    let resolved = resolver.resolve(
        SecretId::OpenRouterApiKey,
        &ResolveOptions {
            include_env: false,
            include_auth_json: false,
            include_populi_env: false,
            profile: ResolveProfile::HardCutStrict,
            caller_context: "test".to_string(),
        },
    );
    assert!(matches!(resolved.status, ResolutionStatus::MissingRequired));
    assert!(resolved.expose().is_none());
    unsafe {
        std::env::remove_var("OPENROUTER_API_KEY");
    }
}

#[test]
#[allow(unsafe_code)]
fn resolved_secret_redaction_never_leaks_raw_value() {
    let _g = ENV_LOCK.lock().expect("env lock");
    unsafe {
        std::env::set_var("OPENAI_API_KEY", "super-secret-value-123456");
    }
    let resolved = resolve_env_only(SecretId::OpenaiApiKey);
    let redacted = resolved.redacted();
    assert!(!redacted.contains("super-secret-value-123456"));
    assert!(redacted.contains("(redacted)") || redacted == "***");
    unsafe {
        std::env::remove_var("OPENAI_API_KEY");
    }
}

struct ChaosBackend {
    counter: AtomicUsize,
}

impl crate::backend::SecretBackend for ChaosBackend {
    fn resolve(
        &self,
        _id: SecretId,
        _spec: crate::spec::SecretSpec,
        _profile: Option<&str>,
        _caller: &str,
    ) -> Result<Option<secrecy::SecretString>, crate::errors::SecretError> {
        let n = self.counter.fetch_add(1, Ordering::Relaxed);
        if n.is_multiple_of(2) {
            Ok(None)
        } else {
            Err(crate::errors::SecretError::BackendUnavailable(
                "chaos backend injected outage".to_string(),
            ))
        }
    }

    fn write_audit_log(
        &self,
        _secret_id: &str,
        _status: &str,
        _resolved_source: Option<&str>,
        _profile: &str,
        _caller_context: &str,
        _detail: Option<&str>,
    ) -> Result<(), crate::errors::SecretError> {
        Ok(())
    }
}

#[test]
fn resolver_chaos_backend_alternates_missing_and_backend_unavailable() {
    let resolver = SecretResolver::new(ChaosBackend {
        counter: AtomicUsize::new(0),
    });
    let mut saw_missing = false;
    let mut saw_unavailable = false;
    for _ in 0..8 {
        let resolved = resolver.resolve(
            SecretId::OpenRouterApiKey,
            &ResolveOptions {
                include_env: false,
                include_auth_json: false,
                include_populi_env: false,
                profile: ResolveProfile::HardCutStrict,
                caller_context: "test".to_string(),
            },
        );
        saw_missing |= matches!(resolved.status, ResolutionStatus::MissingRequired);
        saw_unavailable |= matches!(resolved.status, ResolutionStatus::BackendUnavailable);
    }
    assert!(saw_missing);
    assert!(saw_unavailable);
}

#[test]
#[allow(unsafe_code)]
fn resolver_fuzz_like_env_payloads_never_panic() {
    let _g = ENV_LOCK.lock().expect("env lock");
    use rand::Rng;
    let mut rng = rand::thread_rng();
    for spec in crate::all_specs().iter().take(24) {
        let fuzz_val = (0..32)
            .map(|_| {
                let b = rng.gen_range(33_u8..=126_u8);
                b as char
            })
            .collect::<String>();
        unsafe {
            std::env::set_var(spec.canonical_env, &fuzz_val);
        }
        let resolved = resolve_env_only(spec.id);
        assert!(
            matches!(
                resolved.status,
                ResolutionStatus::Present | ResolutionStatus::DeprecatedAliasUsed
            ),
            "unexpected status {:?} for {}",
            resolved.status,
            spec.canonical_env
        );
        unsafe {
            std::env::remove_var(spec.canonical_env);
        }
    }
}

#[test]
fn cutover_phase_choreography_transitions_as_expected() {
    assert!(crate::CutoverPhase::Shadow.legacy_sources_allowed(ResolveProfile::DevLenient));
    assert!(crate::CutoverPhase::Canary.legacy_sources_allowed(ResolveProfile::DevLenient));
    assert!(!crate::CutoverPhase::Canary.legacy_sources_allowed(ResolveProfile::HardCutStrict));
    assert!(!crate::CutoverPhase::Enforce.legacy_sources_allowed(ResolveProfile::DevLenient));
    assert!(!crate::CutoverPhase::Decommission.legacy_sources_allowed(ResolveProfile::DevLenient));
    assert!(!crate::CutoverPhase::Shadow.force_vox_cloud_backend());
    assert!(crate::CutoverPhase::Decommission.force_vox_cloud_backend());
}

#[test]
#[allow(unsafe_code)]
fn decommission_phase_disables_env_only_fallback_and_forces_vox_cloud() {
    let _g = ENV_LOCK.lock().expect("env lock");
    let prev_cutover = std::env::var("VOX_SECRETS_CUTOVER_PHASE").ok();
    let prev_backend = std::env::var("VOX_SECRETS_BACKEND").ok();
    unsafe {
        std::env::set_var("VOX_SECRETS_CUTOVER_PHASE", "decommission");
        std::env::set_var("VOX_SECRETS_BACKEND", "env_only");
        std::env::set_var("OPENROUTER_API_KEY", "would-be-legacy-fallback");
    }
    let resolved = crate::resolve_secret(SecretId::OpenRouterApiKey);
    assert!(!matches!(resolved.status, ResolutionStatus::Present));
    unsafe {
        match prev_cutover {
            Some(v) => std::env::set_var("VOX_SECRETS_CUTOVER_PHASE", v),
            None => std::env::remove_var("VOX_SECRETS_CUTOVER_PHASE"),
        }
        match prev_backend {
            Some(v) => std::env::set_var("VOX_SECRETS_BACKEND", v),
            None => std::env::remove_var("VOX_SECRETS_BACKEND"),
        }
        std::env::remove_var("OPENROUTER_API_KEY");
    }
}

#[test]
#[allow(unsafe_code)]
fn cutover_phase_compat_alias_is_honored() {
    let _g = ENV_LOCK.lock().expect("env lock");
    let prev_cutover = std::env::var("VOX_SECRETS_CUTOVER_PHASE").ok();
    let prev_migration = std::env::var("VOX_SECRETS_MIGRATION_PHASE").ok();
    unsafe {
        std::env::remove_var("VOX_SECRETS_CUTOVER_PHASE");
        std::env::set_var("VOX_SECRETS_MIGRATION_PHASE", "enforce");
    }
    assert_eq!(
        crate::CutoverPhase::from_env(),
        crate::CutoverPhase::Enforce
    );
    unsafe {
        match prev_cutover {
            Some(v) => std::env::set_var("VOX_SECRETS_CUTOVER_PHASE", v),
            None => std::env::remove_var("VOX_SECRETS_CUTOVER_PHASE"),
        }
        match prev_migration {
            Some(v) => std::env::set_var("VOX_SECRETS_MIGRATION_PHASE", v),
            None => std::env::remove_var("VOX_SECRETS_MIGRATION_PHASE"),
        }
    }
}

#[test]
fn all_secret_ids_have_spec_entries() {
    for &id in &[
        SecretId::GeminiApiKey,
        SecretId::OpenRouterApiKey,
        SecretId::OpenaiApiKey,
        SecretId::AnthropicApiKey,
        SecretId::HuggingFaceToken,
        SecretId::ForgeToken,
        SecretId::GroqApiKey,
        SecretId::CerebrasApiKey,
        SecretId::MistralApiKey,
        SecretId::DeepSeekApiKey,
        SecretId::SambaNovaApiKey,
        SecretId::CustomOpenaiApiKey,
        SecretId::V0ApiKey,
        SecretId::OpenClawToken,
        SecretId::TogetherApiKey,
        SecretId::VoxRunpodApiKey,
        SecretId::VoxVastApiKey,
        SecretId::VoxApiKey,
        SecretId::VoxBearerToken,
        SecretId::VoxDbUrl,
        SecretId::VoxDbToken,
        SecretId::VoxMeshToken,
        SecretId::VoxMeshWorkerToken,
        SecretId::VoxMeshSubmitterToken,
        SecretId::VoxMeshAdminToken,
        SecretId::VoxMeshJwtHmacSecret,
        SecretId::VoxMeshWorkerResultVerifyKey,
        SecretId::VoxNewsTwitterBearer,
        SecretId::VoxNewsOpenCollectiveToken,
        SecretId::VoxSocialRedditClientId,
        SecretId::VoxSocialRedditClientSecret,
        SecretId::VoxSocialRedditRefreshToken,
        SecretId::VoxSocialRedditUserAgent,
        SecretId::VoxSocialYoutubeClientId,
        SecretId::VoxSocialYoutubeClientSecret,
        SecretId::VoxSocialYoutubeRefreshToken,
        SecretId::VoxZenodoAccessToken,
        SecretId::VoxOpenReviewEmail,
        SecretId::VoxOpenReviewAccessToken,
        SecretId::VoxOpenReviewPassword,
        SecretId::VoxCrossrefPlusApiKey,
        SecretId::VoxArxivAssistHandoffSecret,
        SecretId::VoxSearchQdrantApiKey,
        SecretId::PopuliApiKey,
        SecretId::VoxTelemetryUploadUrl,
        SecretId::VoxTelemetryUploadToken,
        SecretId::WebhookIngressToken,
        SecretId::VoxMcpHttpBearerToken,
        SecretId::VoxMcpHttpReadBearerToken,
        SecretId::WebhookSigningSecret,
        SecretId::VoxOrcidClientId,
        SecretId::VoxOrcidClientSecret,
        SecretId::VoxDataCiteRepository,
        SecretId::VoxDataCitePassword,
        SecretId::TavilyApiKey,
        SecretId::TavilyProject,
    ] {
        println!("Checking {:?}", id);
        let _ = id.spec();
    }
}

#[test]
#[allow(unsafe_code)]
fn store_secret_round_trips_user_rsa_nanopub_key_via_temp_vault() {
    // Hermetic: isolate the vault DB to a temp dir via VOX_SECRETS_VAULT_PATH and
    // pin a throwaway VOX_ACCOUNT_ID, mirroring the backend round-trip test in
    // `backend::vox_vault`. Force the vox_cloud backend (cutover=decommission) so
    // that `resolve_secret` reads from the same temp vault that `store_secret`
    // wrote to. The OS keyring holds only the bootstrap master key (shared, not
    // per-secret); if it's unavailable in the sandbox the backend can't init and
    // we skip cleanly.
    let _g = ENV_LOCK.lock().expect("env lock");

    let tmp_dir = tempfile::tempdir().expect("tempdir");
    let db_path = tmp_dir.path().join("store_secret_vault.db");

    let prev_path = std::env::var("VOX_SECRETS_VAULT_PATH").ok();
    let prev_account = std::env::var("VOX_ACCOUNT_ID").ok();
    let prev_cutover = std::env::var("VOX_SECRETS_CUTOVER_PHASE").ok();
    unsafe {
        std::env::set_var("VOX_SECRETS_VAULT_PATH", &db_path);
        std::env::set_var("VOX_ACCOUNT_ID", "store-secret-test-account");
        std::env::set_var("VOX_SECRETS_CUTOVER_PHASE", "decommission");
    }

    const KEY_B64: &str = "MIIEvQIBADANBgkqhkiG9w0BAQEFAA-test-pkcs8-base64-blob";

    let id = SecretId::VoxUserRsaNanopubPrivateKeyB64;
    let stored = crate::store_secret(id, KEY_B64, None);

    let outcome = match stored {
        Ok(()) => {
            let resolved = crate::resolve_secret(id);
            Some(resolved.expose().map(|s| s.to_string()))
        }
        // Keyring/vault unavailable in this sandbox — backend can't init. Skip
        // cleanly only in that case; any other error is a real regression and
        // must fail rather than false-pass.
        Err(e) => {
            let msg = e.to_string().to_lowercase();
            // The vault backend can't initialize in this sandbox when there is no
            // OS keyring / no active runtime — those are documented skip cases.
            // Any other error (e.g. a real write/schema fault) must fail.
            let sandbox_unavailable = [
                "vault",
                "keyring",
                "invalid filename",
                "i/o error",
                "unavailable",
                "backend misconfigured",
                "active tokio runtime",
            ]
            .iter()
            .any(|needle| msg.contains(needle));
            assert!(sandbox_unavailable, "unexpected store_secret failure: {e}");
            None
        }
    };

    unsafe {
        match prev_path {
            Some(v) => std::env::set_var("VOX_SECRETS_VAULT_PATH", v),
            None => std::env::remove_var("VOX_SECRETS_VAULT_PATH"),
        }
        match prev_account {
            Some(v) => std::env::set_var("VOX_ACCOUNT_ID", v),
            None => std::env::remove_var("VOX_ACCOUNT_ID"),
        }
        match prev_cutover {
            Some(v) => std::env::set_var("VOX_SECRETS_CUTOVER_PHASE", v),
            None => std::env::remove_var("VOX_SECRETS_CUTOVER_PHASE"),
        }
    }

    if let Some(exposed) = outcome {
        assert_eq!(
            exposed.as_deref(),
            Some(KEY_B64),
            "store_secret -> resolve_secret must round-trip the stored plaintext"
        );
    }
}

#[test]
fn test_contains_secret_material() {
    let text = "this is a test with a super-secret-value inside";
    assert!(crate::redact::contains_secret_material(
        text,
        &["super-secret-value", "another-secret"]
    ));
    assert!(!crate::redact::contains_secret_material(
        text,
        &["not-in-text", "also-not"]
    ));

    // Short patterns are ignored
    assert!(!crate::redact::contains_secret_material(
        "short",
        &["short"]
    ));
}

#[test]
fn test_redact_secrets_from_value() {
    use serde_json::json;
    let val = json!({
        "data": "my super-secret-value here",
        "nested": ["other-secret-123456", "safe-value"]
    });
    let patterns = vec!["super-secret-value", "other-secret-123456"];
    let scrubbed = crate::redact::redact_secrets_from_value(&val, &patterns);

    let expected = json!({
        "data": "my [REDACTED] here",
        "nested": ["[REDACTED]", "safe-value"]
    });
    assert_eq!(scrubbed, expected);
}

#[test]
fn test_redact_empty_patterns() {
    use serde_json::json;
    let val = json!({"data": "safe"});
    let scrubbed = crate::redact::redact_secrets_from_value(&val, &[]);
    assert_eq!(scrubbed, val);
}

#[test]
fn test_redact_skips_short_patterns() {
    use serde_json::json;
    let val = json!({"data": "short text"});
    let scrubbed = crate::redact::redact_secrets_from_value(&val, &["short"]);
    assert_eq!(scrubbed, val);
}

/// Expo / EAS access token must be a first-class Clavis secret so the mobile
/// toolchain resolves it through the vault (not a bare GitHub Actions secret).
/// TDD red→green for registering EXPO_TOKEN in the secret registry.
#[test]
fn expo_token_is_a_registered_resolvable_secret() {
    use std::str::FromStr;
    let id = crate::SecretId::from_str("EXPO_TOKEN")
        .expect("EXPO_TOKEN must be a registered Clavis secret");
    assert_eq!(
        id.spec().canonical_env,
        "EXPO_TOKEN",
        "EXPO_TOKEN resolves to its own spec"
    );
    assert_eq!(
        id.spec().auth_registry,
        Some("expo"),
        "EXPO_TOKEN is keyed to the `expo` auth registry (vox secrets set expo …)"
    );
    assert!(
        matches!(
            id.metadata().class,
            crate::SecretClass::Integration
        ),
        "EXPO_TOKEN is an Integration-class (persistable, shareable) secret"
    );
}
