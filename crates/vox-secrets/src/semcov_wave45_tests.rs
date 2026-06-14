//! Adversarial unit tests for vox-secrets (semcov wave 4/5).
//! Module: semcov_wave45_tests

#[cfg(test)]
mod semcov_wave45_tests {
    use crate::{
        SecretError,
        policy::{MissingBehavior, SecretPolicy},
        redact::{contains_secret_material, redact_secrets_from_value},
        spec::{
            Profile, RequirementMode, RotationPolicy, SecretBundle, SecretClass, SecretId,
            SecretMaterialKind, SecretSpec, Workflow, all_specs, managed_secret_env_names,
        },
        types::{ResolutionStatus, ResolvedSecret, SecretSource},
    };
    use secrecy::SecretString;
    use serde_json::json;
    use std::str::FromStr;

    // ── SecretId::from_str ────────────────────────────────────────────────────

    #[test]
    fn parse_canonical_env_exact_case() {
        // Catches: from_str skipping canonical_env comparison entirely
        let spec = SecretId::GeminiApiKey.spec();
        let parsed = SecretId::from_str(spec.canonical_env).unwrap();
        assert_eq!(parsed, SecretId::GeminiApiKey);
    }

    #[test]
    fn parse_canonical_env_lowercase_normalised() {
        // Catches: case-normalisation missing for canonical_env path
        let spec = SecretId::AnthropicApiKey.spec();
        let lower = spec.canonical_env.to_lowercase();
        let parsed = SecretId::from_str(&lower).unwrap();
        assert_eq!(parsed, SecretId::AnthropicApiKey);
    }

    #[test]
    fn parse_canonical_env_with_leading_trailing_whitespace() {
        // Catches: trim() not applied before lookup
        let spec = SecretId::OpenRouterApiKey.spec();
        let padded = format!("  {}  ", spec.canonical_env);
        let parsed = SecretId::from_str(&padded).unwrap();
        assert_eq!(parsed, SecretId::OpenRouterApiKey);
    }

    #[test]
    fn parse_unknown_key_returns_err() {
        // Catches: from_str always returning Ok(_) on unknown input
        let result = SecretId::from_str("TOTALLY_NONEXISTENT_KEY_XYZ_9999");
        assert!(result.is_err(), "expected Err for unknown key");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("Unknown SecretId"),
            "error message should mention 'Unknown SecretId'"
        );
    }

    #[test]
    fn parse_empty_string_returns_err() {
        // Catches: empty string accidentally matching a fallback branch
        assert!(SecretId::from_str("").is_err());
    }

    #[test]
    fn parse_debug_repr_roundtrip() {
        // Catches: Debug-repr path in from_str not being case-insensitive
        let id = SecretId::VoxDbToken;
        let debug_str = format!("{:?}", id);
        let parsed = SecretId::from_str(&debug_str).unwrap();
        assert_eq!(parsed, id);
    }

    // ── all_specs / registry completeness ────────────────────────────────────

    #[test]
    fn all_specs_no_duplicate_canonical_env() {
        // Catches: two specs sharing the same canonical_env (silent last-wins behaviour)
        let specs = all_specs();
        let mut seen = std::collections::HashSet::new();
        for spec in &specs {
            let key = spec.canonical_env.to_uppercase();
            assert!(
                seen.insert(key.clone()),
                "duplicate canonical_env detected: {key}"
            );
        }
    }

    #[test]
    fn all_specs_no_duplicate_id() {
        // Catches: two registry slices registering the same SecretId
        let specs = all_specs();
        let mut seen = std::collections::HashSet::new();
        for spec in &specs {
            assert!(
                seen.insert(spec.id),
                "duplicate SecretId in registry: {:?}",
                spec.id
            );
        }
    }

    #[test]
    fn every_id_spec_lookup_succeeds() {
        // Catches: spec() panicking for any variant because registry is missing an entry
        // We exercise a representative sample of variants rather than all 500+.
        let ids = [
            SecretId::GeminiApiKey,
            SecretId::OpenRouterApiKey,
            SecretId::AnthropicApiKey,
            SecretId::VoxDbToken,
            SecretId::VoxMeshToken,
            SecretId::VoxUserRsaNanopubPrivateKeyB64,
        ];
        for id in ids {
            let _ = id.spec(); // must not panic
        }
    }

    #[test]
    fn managed_secret_env_names_non_empty_and_unique() {
        // Catches: managed_secret_env_names returning duplicates or an empty list
        let names = managed_secret_env_names();
        assert!(!names.is_empty());
        let set: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(set.len(), names.len(), "duplicate env names returned");
    }

    // ── SecretMetadata / metadata() correctness ───────────────────────────────

    #[test]
    fn rsa_nanopub_key_is_not_shareable() {
        // Catches: VoxUserRsaNanopubPrivateKeyB64 metadata accidentally marked shareable=true
        let meta = SecretId::VoxUserRsaNanopubPrivateKeyB64.metadata();
        assert!(!meta.shareable, "private RSA key must not be shareable");
    }

    #[test]
    fn rsa_nanopub_key_env_not_allowed_in_strict() {
        // Catches: private signing key allowed from env in strict profile
        let meta = SecretId::VoxUserRsaNanopubPrivateKeyB64.metadata();
        assert!(!meta.allow_env_in_strict);
        assert!(!meta.allow_compat_sources_in_strict);
    }

    #[test]
    fn gemini_api_key_is_integration_class() {
        // Catches: integration API keys accidentally classified as Operator
        let meta = SecretId::GeminiApiKey.metadata();
        assert_eq!(meta.class, SecretClass::Integration);
    }

    #[test]
    fn allows_source_secure_store_always_true_in_strict() {
        // Catches: SecureStore being blocked in strict mode (should always be allowed)
        let meta = SecretId::VoxUserRsaNanopubPrivateKeyB64.metadata();
        assert!(meta.allows_source(SecretSource::SecureStore, true));
    }

    #[test]
    fn allows_source_env_blocked_for_private_key_strict() {
        // Catches: private key metadata allowing env fallback in strict mode
        let meta = SecretId::VoxUserRsaNanopubPrivateKeyB64.metadata();
        assert!(!meta.allows_source(SecretSource::EnvCanonical, true));
        assert!(!meta.allows_source(SecretSource::EnvAlias, true));
    }

    #[test]
    fn allows_source_non_strict_always_true() {
        // Catches: non-strict mode accidentally blocking any source
        let meta = SecretId::VoxUserRsaNanopubPrivateKeyB64.metadata();
        for source in [
            SecretSource::EnvCanonical,
            SecretSource::EnvAlias,
            SecretSource::AuthJson,
            SecretSource::LegacyAuthToken,
            SecretSource::PopuliEnv,
            SecretSource::SecureStore,
            SecretSource::ExternalBackend,
        ] {
            assert!(
                meta.allows_source(source, false),
                "non-strict must allow all sources, failed for {source:?}"
            );
        }
    }

    // ── ResolvedSecret helpers ────────────────────────────────────────────────

    fn make_resolved(val: Option<&str>, status: ResolutionStatus) -> ResolvedSecret {
        ResolvedSecret {
            id: SecretId::GeminiApiKey,
            value: val.map(|s| SecretString::from(s.to_string())),
            source: val.map(|_| SecretSource::EnvCanonical),
            status,
            remediation: "set GEMINI_API_KEY",
            detail: None,
        }
    }

    #[test]
    fn resolved_secret_is_present_with_value() {
        // Catches: is_present() ignoring the value field
        let r = make_resolved(Some("sk-test-1234"), ResolutionStatus::Present);
        assert!(r.is_present());
    }

    #[test]
    fn resolved_secret_is_not_present_without_value() {
        // Catches: is_present() returning true for missing secrets
        let r = make_resolved(None, ResolutionStatus::MissingRequired);
        assert!(!r.is_present());
    }

    #[test]
    fn redacted_shows_missing_when_none() {
        // Catches: redacted() panicking or returning wrong string for None value
        let r = make_resolved(None, ResolutionStatus::MissingOptional);
        assert_eq!(r.redacted(), "(missing)");
    }

    #[test]
    fn redacted_shows_stars_for_short_value() {
        // Catches: redacted() off-by-one on the >6 char threshold
        // exactly 6 chars: "abcdef" → should be "***"
        let r = make_resolved(Some("abcdef"), ResolutionStatus::Present);
        assert_eq!(r.redacted(), "***", "6-char value should produce ***");
    }

    #[test]
    fn redacted_shows_head_tail_for_long_value() {
        // Catches: off-by-one in char counting; head/tail extraction swapping or losing chars
        // "sk-abcde1234" → head=sk-a, tail=34
        let r = make_resolved(Some("sk-abcde1234"), ResolutionStatus::Present);
        let out = r.redacted();
        assert!(out.starts_with("sk-a"), "head mismatch: {out}");
        assert!(out.ends_with("34 (redacted)"), "tail mismatch: {out}");
        assert!(out.contains('…'), "missing ellipsis in {out}");
    }

    #[test]
    fn redacted_round_trip_does_not_expose_secret() {
        // Catches: redacted() accidentally including the full secret in output
        let secret = "super-secret-value-do-not-leak";
        let r = make_resolved(Some(secret), ResolutionStatus::Present);
        let out = r.redacted();
        assert!(
            !out.contains("secret-value-do-not"),
            "redacted output must not contain secret body: {out}"
        );
    }

    // ── redact_secrets_from_value ─────────────────────────────────────────────

    #[test]
    fn redact_replaces_secret_in_json_string() {
        // Catches: redact_secrets_from_value not replacing in leaf string nodes
        let val = json!({"key": "Bearer sk-supersecrettoken"});
        let out = redact_secrets_from_value(&val, &["sk-supersecrettoken"]);
        assert_eq!(out["key"], "[REDACTED]");
    }

    #[test]
    fn redact_does_not_replace_tokens_shorter_than_min_len() {
        // Catches: MIN_REDACT_LEN guard removed, causing tiny strings like "ok" to be redacted
        let val = json!({"msg": "status: ok"});
        let out = redact_secrets_from_value(&val, &["ok"]);
        // "ok" is 2 chars < 8 → must NOT be redacted
        assert_eq!(out["msg"], "status: ok");
    }

    #[test]
    fn redact_replaces_in_nested_json() {
        // Catches: scrub_value_recursive not descending into objects/arrays
        let secret = "my-api-key-1234";
        let val = json!({"outer": {"inner": secret}});
        let out = redact_secrets_from_value(&val, &[secret]);
        assert_eq!(out["outer"]["inner"], "[REDACTED]");
    }

    #[test]
    fn redact_leaves_non_string_values_unchanged() {
        // Catches: numeric/boolean nodes being corrupted by replace_all
        let val = json!({"n": 42, "b": true, "s": "safe"});
        let out = redact_secrets_from_value(&val, &["safe-irrelevant-pattern"]);
        assert_eq!(out["n"], 42);
        assert_eq!(out["b"], true);
    }

    #[test]
    fn redact_empty_patterns_returns_original() {
        // Catches: empty-patterns path cloning incorrectly or panicking
        let val = json!({"k": "value"});
        let out = redact_secrets_from_value(&val, &[]);
        assert_eq!(out, val);
    }

    #[test]
    fn contains_secret_material_true_when_present() {
        // Catches: contains_secret_material always returning false
        assert!(contains_secret_material(
            "Authorization: Bearer sk-verylongtoken",
            &["sk-verylongtoken"]
        ));
    }

    #[test]
    fn contains_secret_material_false_for_short_pattern() {
        // Catches: MIN_REDACT_LEN guard missing in contains_secret_material
        assert!(!contains_secret_material("hello world", &["lo"]));
    }

    // ── SecretPolicy ──────────────────────────────────────────────────────────

    #[test]
    fn secret_policy_required_fail_is_required() {
        // Catches: required_fail() returning required=false
        let p = SecretPolicy::required_fail();
        assert!(p.required);
        assert_eq!(p.behavior, MissingBehavior::Fail);
    }

    #[test]
    fn secret_policy_optional_skip_is_not_required() {
        // Catches: optional_skip() returning required=true
        let p = SecretPolicy::optional_skip();
        assert!(!p.required);
        assert_eq!(p.behavior, MissingBehavior::SkipWithReason);
    }

    // ── SecretError Display ───────────────────────────────────────────────────

    #[test]
    fn secret_error_display_contains_payload() {
        // Catches: error variants swapping or dropping their inner message
        let e = SecretError::BackendUnavailable("vault-down".into());
        assert!(e.to_string().contains("vault-down"), "{e}");

        let e2 = SecretError::BackendMisconfigured("bad-url".into());
        assert!(e2.to_string().contains("bad-url"), "{e2}");

        let e3 = SecretError::Io("permission denied".into());
        assert!(e3.to_string().contains("permission denied"), "{e3}");
    }

    #[test]
    fn secret_error_is_clone() {
        // Catches: derive(Clone) accidentally removed from SecretError
        let e = SecretError::BackendQueryFailed("timeout".into());
        let e2 = e.clone();
        assert_eq!(e.to_string(), e2.to_string());
    }

    // ── SecretBundle ──────────────────────────────────────────────────────────

    #[test]
    fn secret_bundle_doc_names_are_unique() {
        // Catches: two bundles sharing a doc_name (breaks generated docs)
        let mut seen = std::collections::HashSet::new();
        for b in SecretBundle::ALL_VARIANTS {
            assert!(
                seen.insert(b.doc_name()),
                "duplicate doc_name: {}",
                b.doc_name()
            );
        }
    }

    #[test]
    fn secret_bundle_variants_matches_all_variants_len() {
        // Catches: variants() returning fewer entries than ALL_VARIANTS
        assert_eq!(
            SecretBundle::variants().len(),
            SecretBundle::ALL_VARIANTS.len()
        );
    }
}
