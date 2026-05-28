//! Pure-types L0 leaf for vox-orchestrator: agent/task IDs, file affinity, switch actions, provider catalogs.
//!
//! No async runtime, no DB. Consumers wanting just the wire/data shape depend
//! here instead of pulling in the full orchestrator.

// AUTO-GENERATED parts included below.

pub mod agent_types;
pub mod merge_outcome;
pub mod socrates_policy;
pub mod vcs_capability;

pub use vcs_capability::{
    BranchCreate, BranchName, BranchNameError, RemoteId, WorkingTreeWrite, WorkspaceId,
};

pub use agent_types::{
    AccessKind, AgentId, AgentIdGenerator, BatchId, ChangeId, CorrelationId,
    CorrelationIdGenerator, FileAffinity, IdParseError, LockToken, PrioritySource, SnapshotId,
    SnapshotIdGenerator, SwitchAccessMode, SwitchAction, SwitchActionType, TaskId, TaskIdGenerator,
};
pub use merge_outcome::{DaemonId, MergeOutcome};

include!(concat!(env!("OUT_DIR"), "/generated_providers.rs"));

/// Pinned **Inference Endpoint** (dedicated deployment) with an explicit OpenAI-compatible chat URL.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HuggingFaceDedicatedEndpoint {
    pub model: String,
    pub chat_completions_url: String,
    pub bearer_token: Option<String>,
}

/// Resolved router endpoint for chat; bearer token is optional for some public models but usually required.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HuggingFaceRouterEndpoint {
    pub model: String,
    pub chat_completions_url: String,
    pub bearer_token: Option<String>,
}

/// Normalized HTTP/backend lane for chat route telemetry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatRouteBackend {
    /// Google AI Studio / Generative Language API (direct).
    GeminiDirect,
    OpenRouter,
    /// Local Ollama / Mens (`PopuliLocal`).
    Ollama,
    /// Remote Populi mesh node over LAN.
    PopuliMesh,
    /// Aggregators, dedicated endpoints, BYOK OpenAI-compatible, and other non-native lanes.
    CascadeFallback,
    /// Vox-trained MENS checkpoint at 127.0.0.1:7863 (custom wire protocol).
    VoxLocal,
}

/// Resolved high-level route before HTTP client configuration.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind")]
pub enum ChatProviderRouteKind {
    /// Explicit base URL + model (BYOK / custom endpoint).
    ManualOpenAiCompatible {
        base_url: String,
        model: String,
        bearer: Option<String>,
    },
    /// Local Ollama-compatible OpenAI chat API (`{base}/v1/chat/completions`).
    PopuliLocal { base_url: String, model: String },
    /// Remote mesh node API.
    PopuliMesh { base_url: String, model: String },
    /// Hugging Face Inference Providers router (OpenAI-compatible).
    HuggingFaceRouter(HuggingFaceRouterEndpoint),
    /// Pinned HF Inference Endpoint (dedicated deployment).
    HuggingFaceDedicated(HuggingFaceDedicatedEndpoint),
    /// OpenRouter chat completions.
    OpenRouter { model: String },
}

#[must_use]
pub fn route_backend_for_chat_route(route: &ChatProviderRouteKind) -> ChatRouteBackend {
    match route {
        ChatProviderRouteKind::PopuliLocal { .. } => ChatRouteBackend::Ollama,
        ChatProviderRouteKind::PopuliMesh { .. } => ChatRouteBackend::PopuliMesh,
        ChatProviderRouteKind::OpenRouter { .. } => ChatRouteBackend::OpenRouter,
        ChatProviderRouteKind::ManualOpenAiCompatible { base_url, .. } => {
            if base_url
                .to_ascii_lowercase()
                .contains("generativelanguage.googleapis.com")
            {
                ChatRouteBackend::GeminiDirect
            } else {
                ChatRouteBackend::CascadeFallback
            }
        }
        ChatProviderRouteKind::HuggingFaceRouter(_)
        | ChatProviderRouteKind::HuggingFaceDedicated(_) => ChatRouteBackend::CascadeFallback,
    }
}

#[must_use]
pub fn backend_telemetry_labels(backend: ChatRouteBackend) -> (&'static str, &'static str) {
    match backend {
        ChatRouteBackend::GeminiDirect => ("google", "direct"),
        ChatRouteBackend::OpenRouter => ("openrouter", "openrouter"),
        ChatRouteBackend::Ollama => ("mens", "populi_local"),
        ChatRouteBackend::PopuliMesh => ("mens", "populi_mesh"),
        ChatRouteBackend::CascadeFallback => ("custom", "cascade"),
        ChatRouteBackend::VoxLocal => ("vox", "local"),
    }
}
