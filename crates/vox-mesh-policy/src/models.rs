//! Mesh-wide model registry — which LoRA/Ollama tags live on which nodes.
//!
//! This crate is the query layer consumed by the dashboard's model registry view (P4-T12).
//! It does NOT own the data store; nodes report their available models via heartbeat,
//! and this crate provides the query/aggregation types.
pub mod registry;
pub mod types;

pub use registry::ModelRegistry;
pub use types::{ModelEntry, ModelKind, NodeModelMap};
