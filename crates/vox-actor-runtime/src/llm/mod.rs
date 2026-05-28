//! OpenAI-compatible chat, streaming, and embeddings around durable activities.

mod chat;
pub mod cascade;
mod embed;
mod stream;
mod types;
mod wire;

pub use chat::{infer_with_retry, llm_chat};
pub use embed::llm_embed;
pub use stream::llm_stream;
pub use types::{LlmChatMessage, LlmConfig, LlmResponse, ModelMetric, ModelRegistryEntry};
pub use vox_telemetry::{
    FixtureModelIntentResolvedEvent, OrchSubagentDispatchEvent, SubagentDispatchTelemetryPayload,
};
