use async_trait::async_trait;

use crate::checkpoint::CheckpointBundle;
use crate::gradient::GradientShard;

/// Stable identifier for a logical training run (maps to op-log / CAS correlation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct SessionId(pub uuid::Uuid);

impl SessionId {
    #[must_use]
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
pub struct Batch {
    pub batch_id: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StepResult {
    pub step: u64,
    pub loss: f32,
}

#[derive(Debug, thiserror::Error)]
pub enum TrainingError {
    #[error("invalid gradient shard signature")]
    InvalidGradientSignature,
    #[error("invalid checkpoint signature")]
    InvalidCheckpointSignature,
    #[error("gradient rank {got} does not match session rank {expected}")]
    RankMismatch { expected: u32, got: u32 },
    #[error("gradient step {got} does not match expected step {expected}")]
    StepMismatch { expected: u64, got: u64 },
    #[error("all-reduce for world_size={0} is not implemented yet")]
    AllReduceUnsupported(u32),
}

#[async_trait]
pub trait TrainingSession: Send {
    fn rank(&self) -> u32;
    fn world_size(&self) -> u32;
    fn session_id(&self) -> SessionId;
    fn step_index(&self) -> u64;

    async fn step(&mut self, batch: Batch) -> Result<StepResult, TrainingError>;
    async fn all_reduce(&mut self, shard: GradientShard) -> Result<GradientShard, TrainingError>;
    async fn checkpoint(&mut self) -> Result<CheckpointBundle, TrainingError>;
    async fn resume(&mut self, bundle: &CheckpointBundle) -> Result<(), TrainingError>;
}
