//! Core types for the model registry view.

use serde::{Deserialize, Serialize};

/// What kind of model artifact this is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    /// Ollama model tag (e.g. "llama3:8b", "mistral:latest").
    Ollama,
    /// LoRA adapter (e.g. "vox-code-lora-v2").
    Lora,
    /// SafeTensors checkpoint.
    SafeTensors,
    /// Other / unknown.
    Other,
}

impl ModelKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::Lora => "lora",
            Self::SafeTensors => "safetensors",
            Self::Other => "other",
        }
    }

    /// Loose parser used by mesh wire/discovery — never fails because unknown
    /// labels collapse to `Self::Other`. The `from_str` name is kept for
    /// symmetry with `as_str`; `clippy::should_implement_trait` is suppressed
    /// because implementing `FromStr` would require an `Err` type that this
    /// loose mapping never produces.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s {
            "ollama" => Self::Ollama,
            "lora" => Self::Lora,
            "safetensors" => Self::SafeTensors,
            _ => Self::Other,
        }
    }
}

/// A single model/adapter available on one or more nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    /// Human-readable model tag (e.g. "llama3:8b", "vox-code-lora-v2").
    pub tag: String,
    /// Kind of artifact.
    pub kind: ModelKind,
    /// Node IDs that have this model available.
    pub nodes: Vec<String>,
    /// Size in bytes, if reported.
    pub size_bytes: Option<u64>,
    /// SHA-256 digest of the model weights, if available.
    pub digest: Option<String>,
}

/// Per-node model inventory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeModelMap {
    /// Node ID.
    pub node_id: String,
    /// All models available on this node.
    pub models: Vec<ModelEntry>,
}
