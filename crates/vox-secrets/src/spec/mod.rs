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
    registry::missing::SPECS_MISSING,
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
        | SecretId::TavilyProject => &[Capability::AuxTools, Capability::AutonomousResearch],
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
