//! Training run summary type.
//!
//! Ported from `vox-populi/src/mens/tensor/backend.rs` (SP3 sub-batch C).

use serde::{Deserialize, Serialize};

/// Summary of a completed training run.
#[derive(Debug, Serialize, Deserialize)]
pub struct TrainingSummary {
    pub wall_secs: f64,
    pub total_steps: usize,
    pub total_tokens: usize,
    pub ms_per_step: f64,
}
