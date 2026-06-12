//! Internal types for the QLoRA training loop.
//!
//! Ported verbatim from vox-populi (SP3 sub-batch C).

#[derive(Debug, Clone)]
pub struct QloraTrainingResume {
    pub start_epoch: usize,
    pub global_step: u32,
    pub resume_pair_offset: usize,
    pub resume_shuffled_indices: Option<Vec<usize>>,
}

pub struct EncodedTrainStep {
    pub raw_token_len: usize,
    pub ids: Vec<u32>,
    pub prefix_len: usize,
    pub trunc_offset: usize,
    pub sample_weight: f64,
    pub token_weights: Option<Vec<f32>>,
}

pub enum TryEncodeOutcome {
    Encoded(EncodedTrainStep),
    SkipCurriculum,
    SkipShortSeq,
}

pub enum MaskedCeForward {
    NoSupervision,
    NonFinite {
        kind: &'static str,
        mask_sum: f32,
    },
    Finite {
        loss: candle_core::Tensor,
        loss_scalar: f32,
        supervised_tokens: u64,
        theoretical_tokens: u64,
        syntax_weight_sum: f32,
        /// Checkpoint segments when the forward was run with activation
        /// checkpointing. `None` for the eager path. When `Some`, the loss's
        /// autograd tape spans only the final segment + head; the training loop
        /// must run the segmented recompute-backward (threading the boundary
        /// cotangent in reverse) rather than a single `backward_step`.
        segments: Option<Vec<crate::model::CheckpointSegment>>,
    },
}
