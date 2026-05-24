//! Distributed training primitives for the MENS mesh (Mn-T1, Mn-T6).
//!
//! CUDA-only training execution remains in `vox-plugin-mens-candle-cuda`; this crate owns the
//! **cross-host contracts**: gradient envelopes, signed checkpoint bundles, and op-log routing.

mod checkpoint;
mod gradient;
pub mod mesh_env;
mod session;
pub mod strategy;
pub mod telemetry;

pub use checkpoint::CheckpointBundle;
pub use gradient::GradientShard;
pub use mesh_env::{MeshTrainConfig, get_mesh_rank, is_mesh_mode};
pub use session::{Batch, SessionId, StepResult, TrainingError, TrainingSession};
pub use strategy::data_parallel::DataParallelSession;
