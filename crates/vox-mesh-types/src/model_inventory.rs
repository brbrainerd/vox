use serde::{Deserialize, Serialize};

/// A single model available on a specific mesh node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInventoryEntry {
    /// Canonical model identifier (e.g. `"meta-llama/Llama-3-8B-Instruct"`).
    pub model_id: String,
    /// The node advertising this model.
    pub node_id: String,
    /// VRAM required to load the model, in mebibytes.
    pub vram_mb: u32,
    /// Quantization scheme applied (e.g. `"q4_0"`, `"fp16"`, `"none"`).
    pub quantization: String,
}

/// Mesh-wide snapshot of all advertised models across all visible nodes.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MeshModelInventory {
    /// All model entries collected during the snapshot sweep.
    pub entries: Vec<ModelInventoryEntry>,
    /// Unix epoch milliseconds when this snapshot was collected.
    pub snapshot_unix_ms: u64,
}
