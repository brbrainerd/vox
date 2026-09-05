//! OpenAI-compatible chat, streaming, and embeddings around durable activities.

pub mod cascade;
mod chat;
mod embed;
mod stream;
mod types;

/// The per-provider AIMD throttle now lives in `vox-llm-egress` (the sanctioned egress
/// core). Re-exported here so existing `vox_actor_runtime::llm::throttle::*` paths keep working.
pub use vox_llm_egress::throttle;

pub use chat::{
    CONTEXT_EXCEEDED_PREFIX, RATE_LIMITED_ERROR_CLASS, RATE_LIMITED_PREFIX, infer_with_retry,
    llm_chat,
};
pub use embed::llm_embed;
pub use stream::{llm_stream, llm_stream_activity};
pub use types::{
    LlmChatMessage, LlmConfig, LlmResponse, LlmToolDef, ModelMetric, ModelRegistryEntry,
};
pub use vox_telemetry::{
    FixtureModelIntentResolvedEvent, OrchSubagentDispatchEvent, SubagentDispatchTelemetryPayload,
};
