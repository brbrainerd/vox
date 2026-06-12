//! Pluggable inference backends for mesh-routed model execution (Mn-T2).
//!
//! Training contracts live in [`super::distributed_training`]; this module models **inference**
//! dispatch across heterogeneous devices.

mod backend;
pub mod backends;
mod dispatcher;
pub mod generate;
pub mod qwen_forward;
pub mod qwen_weights;
pub mod swarm;

pub use backend::{
    BackendCapabilities, BackendId, InferenceBackend, InferenceError, LoadedModel, PromptInput,
    Quantization, SamplingParams, Verdict,
};
pub use backends::{
    CandleCpuBackend, CandleCudaBackend, CandleMetalBackend, LlamaCppRpcBackend,
    OllamaSubprocessBackend,
};
pub use dispatcher::InferenceDispatcher;
