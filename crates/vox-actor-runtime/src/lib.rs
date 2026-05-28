//! Process-oriented runtime primitives for Vox: actors, mailboxes, supervision, and scheduling.
//!
//! Submodules implement the durable / distributed process story; this crate root mostly re-exports
//! stable names for embedders. Detailed behavior is documented on the defining types inside each
//! module; blanket Rustdoc on every `pub use` alias would add noise without semantic value.
#![allow(clippy::collapsible_if)]

/// Durable activity execution: retries, timeouts, and backoff around async work.
pub mod activity;
/// Host-callable builtins surfaced to generated code (hashing, small helpers).
pub mod builtins;
#[cfg(feature = "database")]
/// Optional Codex / Turso database handle when the `database` feature is enabled.
pub mod db;
/// Durable / cron scheduler primitives for `@scheduled` + `@durable` (GA-11).
pub mod durable_scheduler;
/// Garbage collection arena and !Send pointers for actor memory isolation.
pub mod gc;
/// Hugging Face router, Hub listings, and Mens/Ollama capability probes.
pub mod inference_env;
/// OpenAI-compatible chat/embed clients, registry entries, and usage metrics.
pub mod llm;
/// Typed result wrapper for structured LLM activity returns (parse-safe).
pub mod llm_result;
/// Actor mailboxes: envelopes, messages, requests, and process signals.
pub mod mailbox;
/// Mens/Ollama HTTP client for generate, embed, classify, and RAG helpers.
pub mod mens;
/// SSOT chat routing: manual URL, Mens, HF dedicated/router, OpenRouter.
pub mod model_resolution;
/// Multi-channel notification dispatch (GA-14): Email / SMS / WebPush.
pub mod notify;
/// Opaque process identifiers for actors and messaging.
pub mod pid;
/// Cursor-presence channel for real-time multiplayer (GA-25).
pub mod presence;
/// Actor `ProcessContext`, `ProcessHandle`, and `spawn_process`.
pub mod process;
/// Prompt normalization, conflict detection, and safety pass for LLM ingress.
pub mod prompt_canonical;
/// Global map of live actors by [`Pid`] and optional name.
pub mod registry;
/// Retry policy and HTTP client with fallback endpoints.
pub mod resilient_http;
/// RAG-style chunk retrieval, context budgets, and provenance records.
pub mod retrieval;
/// `VOX_ROUTE_*` capability policy snapshot (shared with orchestrator explain/MCP paths).
pub mod route_capability_policy;
/// Shared routing decision telemetry for DB joins (bounded `reason_json`).
pub mod routing_telemetry;
/// Cooperative Tokio-backed scheduler registering spawned actors.
pub mod scheduler;
/// Reactive state-machine instance helper consumed by emitted state-machine hooks
/// (Phase G of the Svelte-mineable features plan).
pub mod state_machine;
/// Per-table reactive mutation notifications (broadcast channels for reactive queries).
pub mod subscription;
/// Supervision strategies and child restart loops for actor trees.
pub mod supervisor;

pub use activity::{ActivityError, ActivityOptions, ActivityResult, execute_activity};
pub use llm_result::{LlmError, LlmResult, StdLlmResult};
pub use mailbox::{Envelope, Message, MessagePayload, Request};
pub use pid::Pid;
pub use process::{CallError, ProcessContext, ProcessHandle, spawn_process};
pub use registry::{ActorHandler, ActorRegistry, ProcessRegistry, RegistryError};
pub use resilient_http::RetryPolicy;
pub use retrieval::{ContextBudget, ProvenanceRecord, RetrievedChunk, apply_context_budget};
pub use subscription::SubscriptionManager;

/// Re-exported pure backoff math ([`vox_foundation::primitives::backoff`]).
pub use vox_foundation::primitives::backoff;
/// Re-exported cheap hex ids ([`vox_foundation::primitives::id`]).
pub use vox_foundation::primitives::id as simple_id;
