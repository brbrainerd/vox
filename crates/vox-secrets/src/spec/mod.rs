mod types;
pub use types::*;
mod ids;
pub use ids::*;
pub mod registry;

use std::collections::BTreeSet;

pub const ALL_REGISTRIES: &[&[SecretSpec]] = &[
    registry::SPECS_LLM,
    registry::SPECS_PLATFORM,
    registry::SPECS_MESH,
    registry::SPECS_SOCIAL,
    registry::SPECS_SCHOLARLY,
    registry::SPECS_CONFIG,
    registry::SPECS_IDENTITY,
    registry::missing::SPECS_MISSING_CORE,
    registry::missing::SPECS_MISSING_TAIL,
];

#[must_use]
pub fn all_specs() -> Vec<&'static SecretSpec> {
    let mut out = Vec::new();
    for reg in ALL_REGISTRIES {
        for spec in *reg {
            out.push(spec);
        }
    }
    out
}

#[must_use]
pub fn managed_secret_env_names() -> Vec<&'static str> {
    let mut names = BTreeSet::new();
    for reg in ALL_REGISTRIES {
        for spec in *reg {
            names.insert(spec.canonical_env);
            for alias in spec.aliases {
                names.insert(*alias);
            }
            for alias in spec.deprecated_aliases {
                names.insert(*alias);
            }
        }
    }
    names.into_iter().collect()
}

pub fn requirements_for_profile(workflow: Workflow, profile: Profile) -> WorkflowRequirements {
    requirements_for_profile_mode(workflow, profile, RequirementMode::Auto)
}

pub fn requirements_for_profile_mode(
    workflow: Workflow,
    profile: Profile,
    mode: RequirementMode,
) -> WorkflowRequirements {
    let effective_mode = match mode {
        RequirementMode::Auto => match profile {
            Profile::Dev | Profile::Mobile => RequirementMode::Local,
            Profile::Ci | Profile::Prod => RequirementMode::Cloud,
        },
        RequirementMode::Local => RequirementMode::Local,
        RequirementMode::Cloud => RequirementMode::Cloud,
    };

    let mut blocking = match workflow {
        Workflow::Chat | Workflow::Mcp => match effective_mode {
            RequirementMode::Local | RequirementMode::Auto => vec![],
            RequirementMode::Cloud => vec![RequirementSet::AllOf(CHAT_CLOUD_PRIMARY)],
        },
        Workflow::Publish | Workflow::Review => {
            vec![RequirementSet::AllOf(&[SecretId::ForgeToken])]
        }
        Workflow::DbRemote => vec![RequirementSet::AllOf(&[
            SecretId::VoxDbUrl,
            SecretId::VoxDbToken,
        ])],
        Workflow::MensMesh => vec![RequirementSet::AnyOf(&[
            SecretId::VoxMeshToken,
            SecretId::VoxMeshWorkerToken,
            SecretId::VoxMeshSubmitterToken,
            SecretId::VoxMeshAdminToken,
        ])],
    };

    if matches!(profile, Profile::Ci) && matches!(workflow, Workflow::Chat | Workflow::Mcp) {
        blocking.push(RequirementSet::AllOf(&[SecretId::ForgeToken]));
    }

    let optional = match workflow {
        Workflow::Chat | Workflow::Mcp => ALL_CHAT_OPTIONALS.to_vec(),
        Workflow::Publish | Workflow::Review => vec![
            SecretId::VoxZenodoAccessToken,
            SecretId::VoxOpenReviewEmail,
            SecretId::VoxOpenReviewAccessToken,
            SecretId::VoxOpenReviewPassword,
            SecretId::VoxCrossrefPlusApiKey,
            SecretId::VoxArxivAssistHandoffSecret,
            SecretId::VoxOrcidClientId,
            SecretId::VoxOrcidClientSecret,
            SecretId::VoxDataCiteRepository,
            SecretId::VoxDataCitePassword,
        ],
        Workflow::DbRemote => vec![],
        Workflow::MensMesh => vec![
            SecretId::VoxMeshJwtHmacSecret,
            SecretId::VoxMeshWorkerResultVerifyKey,
            SecretId::PopuliApiKey,
        ],
    };
    WorkflowRequirements { blocking, optional }
}

pub fn requirements_for_bundle(bundle: SecretBundle) -> WorkflowRequirements {
    match bundle {
        SecretBundle::MinimalLocalDev => WorkflowRequirements {
            blocking: vec![],
            optional: ALL_CHAT_OPTIONALS.to_vec(),
        },
        SecretBundle::MinimalCloudDev => WorkflowRequirements {
            blocking: vec![RequirementSet::AllOf(CHAT_CLOUD_PRIMARY)],
            optional: ALL_CHAT_OPTIONALS.to_vec(),
        },
        SecretBundle::GpuCloud => WorkflowRequirements {
            blocking: vec![RequirementSet::AnyOf(&[
                SecretId::VoxRunpodApiKey,
                SecretId::VoxVastApiKey,
            ])],
            optional: vec![SecretId::TogetherApiKey],
        },
        SecretBundle::PublishReview => WorkflowRequirements {
            blocking: vec![RequirementSet::AllOf(&[SecretId::ForgeToken])],
            optional: vec![],
        },
        SecretBundle::MeshRoles => WorkflowRequirements {
            blocking: vec![RequirementSet::AnyOf(&[
                SecretId::VoxMeshWorkerToken,
                SecretId::VoxMeshSubmitterToken,
            ])],
            optional: vec![SecretId::VoxMeshToken, SecretId::VoxMeshAdminToken],
        },
    }
}

pub fn all_bundle_doc_names() -> &'static [&'static str] {
    &[
        "minimal_local_dev",
        "minimal_cloud_dev",
        "gpu_cloud",
        "publish_review",
        "mesh_roles",
    ]
}

pub fn capabilities_for_secret(id: SecretId) -> &'static [Capability] {
    match id {
        SecretId::OpenRouterApiKey => &[Capability::ChatCloudPrimary],
        SecretId::GeminiApiKey
        | SecretId::OpenaiApiKey
        | SecretId::AnthropicApiKey
        | SecretId::GroqApiKey
        | SecretId::CerebrasApiKey
        | SecretId::MistralApiKey
        | SecretId::DeepSeekApiKey
        | SecretId::SambaNovaApiKey
        | SecretId::CustomOpenaiApiKey
        | SecretId::HuggingFaceToken => &[Capability::ChatCloudAlt],
        SecretId::VoxRunpodApiKey | SecretId::VoxVastApiKey | SecretId::TogetherApiKey => {
            &[Capability::GpuCloud]
        }
        SecretId::ForgeToken => &[Capability::PublishReview],
        SecretId::VoxDbUrl | SecretId::VoxDbToken => &[Capability::DbRemote],
        SecretId::VoxMeshToken
        | SecretId::VoxMeshWorkerToken
        | SecretId::VoxMeshSubmitterToken
        | SecretId::VoxMeshAdminToken
        | SecretId::VoxMeshJwtHmacSecret
        | SecretId::VoxMeshWorkerResultVerifyKey
        | SecretId::VoxMeshFederationSigningKey
        | SecretId::VoxRoutingPreferMesh => &[Capability::Mesh],
        SecretId::VoxApiKey | SecretId::VoxBearerToken => &[Capability::RuntimeIngress],
        SecretId::V0ApiKey
        | SecretId::OpenClawToken
        | SecretId::TavilyApiKey
        | SecretId::TavilyProject
        | SecretId::SonatypeGuideToken => &[Capability::AuxTools, Capability::AutonomousResearch],
        SecretId::VoxNewsTwitterBearer
        | SecretId::VoxSocialBlueskyHandle
        | SecretId::VoxSocialBlueskyPassword
        | SecretId::VoxNewsOpenCollectiveToken
        | SecretId::VoxSocialRedditClientId
        | SecretId::VoxSocialRedditClientSecret
        | SecretId::VoxSocialRedditRefreshToken
        | SecretId::VoxSocialRedditUserAgent
        | SecretId::VoxSocialYoutubeClientId
        | SecretId::VoxSocialYoutubeClientSecret
        | SecretId::VoxSocialYoutubeRefreshToken
        | SecretId::VoxSocialMastodonToken
        | SecretId::VoxSocialMastodonDomain
        | SecretId::VoxSocialLinkedinAccessToken
        | SecretId::VoxSocialLinkedinAuthorUrn
        | SecretId::VoxSocialBlueskyPdsUrl
        | SecretId::VoxNewsOpenCollectiveSlug
        | SecretId::VoxSocialDiscordWebhook
        | SecretId::VoxOpenRouterClassifierEnabled => &[
            Capability::ScientiaSyndication,
            Capability::AutonomousResearch,
        ],
        SecretId::VoxZenodoAccessToken
        | SecretId::VoxOpenReviewEmail
        | SecretId::VoxOpenReviewAccessToken
        | SecretId::VoxOpenReviewPassword
        | SecretId::VoxCrossrefPlusApiKey
        | SecretId::VoxArxivAssistHandoffSecret
        | SecretId::VoxOrcidClientId
        | SecretId::VoxOrcidClientSecret
        | SecretId::VoxDataCiteRepository
        | SecretId::VoxDataCitePassword => &[Capability::ScholarlyPublication],
        SecretId::VoxSearchQdrantApiKey
        | SecretId::PopuliApiKey
        | SecretId::VoxTelemetryUploadUrl
        | SecretId::VoxTelemetryUploadToken => &[Capability::AuxTools],
        _ => &[Capability::Orchestration],
    }
}

/// The canonical taxonomy class for a secret. Derives from the secret's primary
/// capability (the classification SSOT), with numeric tuning knobs forced to
/// `OperatorTuning` so they stay out of the credential surfaces. This is the
/// single source the GUI panel grouping and the GUI search codegen both read.
#[must_use]
pub fn taxonomy_class_for(id: SecretId) -> TaxonomyClass {
    // Tuning knobs (e.g. GEMINI_TUNING_TEMPERATURE) live in the secret registry
    // but are operator config, not credentials.
    if id.spec().canonical_env.contains("_TUNING_") {
        return TaxonomyClass::OperatorTuning;
    }
    match capabilities_for_secret(id).first() {
        Some(cap) => TaxonomyClass::from_capability(*cap),
        None => TaxonomyClass::AuxTooling,
    }
}

/// Whether a spec is a real, user-facing credential worth surfacing in the GUI
/// (panel + search), as opposed to a non-credential env placeholder that exists
/// only for `managed_secret_env_names()` parity.
///
/// A spec is user-facing if it is a persistable account credential
/// (`metadata().persistable_account_secret` — true for every curated credential
/// incl. the binding-less identity tokens), has a vault/keyring storage binding,
/// or carries a curated (non-default) capability. Non-credential env placeholders
/// and config knobs fall to the `Operator`/`Orchestration` defaults with no
/// binding, so they are excluded. Config-only tuning knobs are never shown.
///
/// Note: `scope_description`/`remediation` are deliberately NOT signals here —
/// config knobs (e.g. "Repository root path") also carry descriptions.
#[must_use]
pub fn is_user_facing_secret(id: SecretId) -> bool {
    if taxonomy_class_for(id).is_config_only() {
        return false;
    }
    let spec = id.spec();
    id.metadata().persistable_account_secret
        || spec.auth_registry.is_some()
        || spec.backend_key.is_some()
        || !matches!(
            capabilities_for_secret(id).first(),
            Some(Capability::Orchestration) | None
        )
}

pub fn required_for(workflow: Workflow) -> Vec<SecretId> {
    required_for_profile(workflow, Profile::Dev)
}

pub fn required_for_profile(workflow: Workflow, profile: Profile) -> Vec<SecretId> {
    let mut out = BTreeSet::new();
    for req in requirements_for_profile(workflow, profile).blocking {
        match req {
            RequirementSet::AnyOf(ids) | RequirementSet::AllOf(ids) => {
                for id in ids {
                    out.insert(*id);
                }
            }
        }
    }
    out.into_iter().collect()
}

pub const fn secret_reads_populi_env_file(id: SecretId) -> bool {
    matches!(
        id,
        SecretId::VoxMeshToken
            | SecretId::VoxMeshWorkerToken
            | SecretId::VoxMeshSubmitterToken
            | SecretId::VoxMeshAdminToken
            | SecretId::VoxMeshScopeId
    )
}

#[cfg(test)]
mod taxonomy_tests {
    use super::*;

    #[test]
    fn taxonomy_is_not_degenerate() {
        // Known providers must classify into their real categories.
        assert_eq!(taxonomy_class_for(SecretId::GeminiApiKey).slug(), "llm");
        assert_eq!(taxonomy_class_for(SecretId::VoxRunpodApiKey).slug(), "gpu");
        assert_eq!(
            taxonomy_class_for(SecretId::VoxSocialRedditClientId).slug(),
            "social"
        );
        // Tuning knobs are operator config, not credentials.
        assert!(taxonomy_class_for(SecretId::GeminiTuningTemperature).is_config_only());

        // Distribution is measured over the SURFACED (user-facing) set — the
        // ~423 non-credential placeholders are excluded, so categories must be
        // genuinely diverse, not dominated by one catch-all bucket.
        let surfaced: Vec<_> = all_specs()
            .into_iter()
            .filter(|s| is_user_facing_secret(s.id))
            .collect();
        let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
        for s in &surfaced {
            *counts.entry(taxonomy_class_for(s.id).slug()).or_default() += 1;
        }
        let total = surfaced.len();
        let max = counts.values().copied().max().unwrap_or(0);
        assert!(
            (40..=200).contains(&total),
            "surfaced credential count {total} outside sane range — filter over/under-matched: {counts:?}"
        );
        assert!(
            counts.len() >= 6,
            "expected >=6 distinct taxonomy slugs, got {}: {:?}",
            counts.len(),
            counts
        );
        assert!(
            max * 100 / total <= 70,
            "one slug holds {}% of surfaced specs (degenerate): {:?}",
            max * 100 / total,
            counts
        );
    }

    #[test]
    fn real_credentials_surface_and_placeholders_do_not() {
        // Real, curated credentials must be surfaced...
        for id in [
            SecretId::GeminiApiKey,
            SecretId::OpenRouterApiKey,
            SecretId::VoxGithubOauthToken, // identity: no binding, but has scope+remediation
            SecretId::VoxRunpodApiKey,
            SecretId::VoxSocialRedditClientId,
        ] {
            assert!(is_user_facing_secret(id), "{id:?} should be surfaced");
        }
        // ...and non-credential env placeholders must not.
        for id in [SecretId::VoxMeshEnabled, SecretId::VoxMeshMode] {
            assert!(!is_user_facing_secret(id), "{id:?} should be filtered out");
        }
    }
}

#[cfg(test)]
mod uniqueness_tests {
    use super::*;
    use std::collections::BTreeMap;

    /// `SecretId::spec()` (see `spec/ids.rs`) does a linear scan over
    /// `ALL_REGISTRIES` and returns the first match. Any duplicate
    /// `SecretId` registration is therefore unreachable dead code at best
    /// and a silent resolution-divergence bug at worst (the second entry's
    /// canonical_env / aliases / policy are simply ignored). Five such
    /// duplicates were removed on 2026-05-02 (`TavilyProject`,
    /// `VoxGithubSha`, `SkipCudaFeatureCheck`, `VoxCargoBin`,
    /// `VoxCliGlobalJson`); this test stops them from coming back.
    #[test]
    fn every_secret_id_is_registered_at_most_once() {
        let mut counts: BTreeMap<SecretId, Vec<&'static str>> = BTreeMap::new();
        for reg in ALL_REGISTRIES {
            for spec in *reg {
                counts.entry(spec.id).or_default().push(spec.canonical_env);
            }
        }
        let dups: Vec<_> = counts
            .iter()
            .filter(|(_, v)| v.len() > 1)
            .map(|(id, envs)| format!("{id:?} appears {} times: {envs:?}", envs.len()))
            .collect();
        assert!(
            dups.is_empty(),
            "duplicate SecretId registrations (only the first wins at runtime):\n  {}",
            dups.join("\n  ")
        );
    }
}
