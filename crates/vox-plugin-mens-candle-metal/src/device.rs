//! Device selection for Candle training.
//!
//! Ported from `vox-populi/src/mens/tensor/device.rs` (SP3 sub-batch C).
//! `probe_gpu` is stubbed to avoid pulling in vox-populi hardware registry.

/// CLI / env device intent for the Candle backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    /// Prefer host CPU / software paths.
    Cpu,
    /// Let the stack pick (Candle Metal → CPU when available).
    #[default]
    Best,
    /// Prefer NVIDIA CUDA.
    Cuda,
    /// Prefer Apple Metal (macOS).
    Metal,
}

/// Best-effort local GPU description.
#[derive(Debug, Clone)]
pub struct GpuInfo {
    pub model_name: String,
    pub vram_mb: u64,
    pub vendor: String,
}

/// Minimal GPU probe — returns unknown vendor when no probe is possible.
/// SP3-C stub: hardware registry is a vox-populi concern; reconnect via host capability in sub-batch D.
#[must_use]
pub fn probe_gpu() -> GpuInfo {
    GpuInfo {
        model_name: "unknown".to_string(),
        vram_mb: 0,
        vendor: "unknown".to_string(),
    }
}
