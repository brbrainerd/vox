use super::ids::SecretId;
use crate::policy::SecretPolicy;
use crate::types::SecretSource;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaxonomyClass {
    PlatformIdentity,
    LlmProviderKey,
    CloudGpuInfra,
    ScholarlyPublication,
    SocialSyndication,
    MeshTransport,
    TelemetrySearch,
    AuxTooling,
    OperatorTuning,
}

impl TaxonomyClass {
    pub const fn slug(self) -> &'static str {
        match self {
            Self::PlatformIdentity => "platform",
            Self::LlmProviderKey => "llm",
            Self::CloudGpuInfra => "gpu",
            Self::ScholarlyPublication => "scholarly",
            Self::SocialSyndication => "social",
            Self::MeshTransport => "mesh",
            Self::TelemetrySearch => "telemetry",
            Self::AuxTooling => "aux",
            Self::OperatorTuning => "config",
        }
    }

    pub const fn is_config_only(self) -> bool {
        matches!(self, Self::OperatorTuning)
    }

    /// Map a secret's primary [`Capability`] to its taxonomy class. This is the
    /// single classification SSOT: `capabilities_for_secret` already groups every
    /// `SecretId`, so taxonomy is derived from it rather than re-enumerated.
    pub const fn from_capability(cap: Capability) -> Self {
        match cap {
            Capability::ChatCloudPrimary | Capability::ChatCloudAlt => Self::LlmProviderKey,
            Capability::GpuCloud => Self::CloudGpuInfra,
            Capability::Mesh => Self::MeshTransport,
            Capability::ScholarlyPublication => Self::ScholarlyPublication,
            Capability::ScientiaSyndication => Self::SocialSyndication,
            Capability::DbRemote => Self::TelemetrySearch,
            Capability::RuntimeIngress | Capability::PublishReview | Capability::Orchestration => {
                Self::PlatformIdentity
            }
            Capability::AuxTools | Capability::AutonomousResearch => Self::AuxTooling,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LifecycleMeta {
    pub rotation_cadence_days: Option<u32>,
    pub expiry_warning_days: Option<u32>,
    pub track_stale_rotation: bool,
}

impl LifecycleMeta {
    pub const MANUAL: Self = Self {
        rotation_cadence_days: None,
        expiry_warning_days: None,
        track_stale_rotation: false,
    };
    pub const QUARTERLY: Self = Self {
        rotation_cadence_days: Some(90),
        expiry_warning_days: Some(14),
        track_stale_rotation: true,
    };
    pub const MONTHLY: Self = Self {
        rotation_cadence_days: Some(30),
        expiry_warning_days: Some(7),
        track_stale_rotation: true,
    };
    pub const ANNUAL_OAUTH: Self = Self {
        rotation_cadence_days: Some(365),
        expiry_warning_days: Some(30),
        track_stale_rotation: true,
    };
    pub const CONFIG: Self = Self {
        rotation_cadence_days: None,
        expiry_warning_days: None,
        track_stale_rotation: false,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecretClass {
    Runtime,
    Account,
    Operator,
    Integration,
    Transport,
    Bootstrap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecretMaterialKind {
    ApiKey,
    OAuthRefreshToken,
    BearerToken,
    HmacSecret,
    EndpointUrl,
    Username,
    Password,
    OAuthClientCredential,
    JwtHmacSecret,
    Ed25519Key,
    DelegationRef,
    ConfigValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RotationPolicy {
    Manual,
    Periodic,
    PerIncident,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SecretMetadata {
    pub class: SecretClass,
    pub material_kind: SecretMaterialKind,
    pub persistable_account_secret: bool,
    pub shareable: bool,
    pub device_local_only: bool,
    pub allow_env_in_strict: bool,
    pub allow_compat_sources_in_strict: bool,
    pub rotation_policy: RotationPolicy,
    pub taxonomy_class: TaxonomyClass,
    pub lifecycle: LifecycleMeta,
}

impl SecretMetadata {
    #[must_use]
    pub const fn allows_source(self, source: SecretSource, strict_profile: bool) -> bool {
        if !strict_profile {
            return true;
        }
        match source {
            SecretSource::EnvCanonical | SecretSource::EnvAlias => self.allow_env_in_strict,
            SecretSource::AuthJson | SecretSource::LegacyAuthToken | SecretSource::PopuliEnv => {
                self.allow_compat_sources_in_strict
            }
            SecretSource::SecureStore | SecretSource::ExternalBackend => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Workflow {
    Chat,
    Mcp,
    Publish,
    Review,
    DbRemote,
    MensMesh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Profile {
    Dev,
    Ci,
    Mobile,
    Prod,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RequirementMode {
    Auto,
    Local,
    Cloud,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecretBundle {
    MinimalLocalDev,
    MinimalCloudDev,
    GpuCloud,
    PublishReview,
    MeshRoles,
}

impl SecretBundle {
    pub const ALL_VARIANTS: &'static [Self] = &[
        Self::MinimalLocalDev,
        Self::MinimalCloudDev,
        Self::GpuCloud,
        Self::PublishReview,
        Self::MeshRoles,
    ];

    #[must_use]
    pub const fn variants() -> &'static [Self] {
        Self::ALL_VARIANTS
    }

    #[must_use]
    pub const fn doc_name(self) -> &'static str {
        match self {
            Self::MinimalLocalDev => "minimal_local_dev",
            Self::MinimalCloudDev => "minimal_cloud_dev",
            Self::GpuCloud => "gpu_cloud",
            Self::PublishReview => "publish_review",
            Self::MeshRoles => "mesh_roles",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    ChatCloudPrimary,
    ChatCloudAlt,
    GpuCloud,
    PublishReview,
    DbRemote,
    Mesh,
    RuntimeIngress,
    AuxTools,
    Orchestration,
    ScientiaSyndication,
    ScholarlyPublication,
    AutonomousResearch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RequirementSet {
    AnyOf(&'static [SecretId]),
    AllOf(&'static [SecretId]),
}

#[derive(Debug, Clone)]
pub struct WorkflowRequirements {
    pub blocking: Vec<RequirementSet>,
    pub optional: Vec<SecretId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SecretSpec {
    pub id: SecretId,
    pub canonical_env: &'static str,
    pub aliases: &'static [&'static str],
    pub deprecated_aliases: &'static [&'static str],
    pub backend_key: Option<&'static str>,
    pub auth_registry: Option<&'static str>,
    pub policy: SecretPolicy,
    pub remediation: &'static str,
    pub scope_description: &'static str,
}
