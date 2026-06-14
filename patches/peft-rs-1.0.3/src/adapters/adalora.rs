//! `AdaLoRA` (Adaptive Low-Rank Adaptation) implementation.
//!
//! `AdaLoRA` dynamically allocates rank budget during training using SVD-based
//! importance scores. It uses a three-phase training schedule:
//! 1. Initial warmup phase (tinit steps)
//! 2. Rank reduction phase (between tinit and `total_step` - tfinal)
//! 3. Final fine-tuning phase (tfinal steps)
//!
//! Reference: <https://arxiv.org/abs/2303.10512>

#![allow(clippy::doc_markdown)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::uninlined_format_args)]

use candle_core::{DType, Device, Tensor};
use candle_nn::VarMap;
use serde::{Deserialize, Serialize};

use crate::error::{PeftError, Result};
use crate::traits::{Adapter, AdapterConfig, Mergeable, Trainable};

/// Configuration for AdaLoRA adapters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdaLoraConfig {
    /// Target average rank after pruning.
    pub target_r: usize,

    /// Initial rank for each incremental matrix (before pruning).
    pub init_r: usize,

    /// Scaling factor (typically alpha / r).
    pub alpha: usize,

    /// Dropout probability applied to outputs.
    #[serde(default)]
    pub dropout: f64,

    /// Target modules to apply AdaLoRA to.
    #[serde(default = "default_target_modules")]
    pub target_modules: Vec<String>,

    /// Steps of initial warmup (no rank reduction).
    #[serde(default)]
    pub tinit: usize,

    /// Steps of final fine-tuning (no rank reduction).
    #[serde(default)]
    pub tfinal: usize,

    /// Time interval between budget allocations.
    #[serde(default = "default_delta_t")]
    pub delta_t: usize,

    /// Hyperparameter of EMA for sensitivity smoothing.
    #[serde(default = "default_beta")]
    pub beta1: f64,

    /// Hyperparameter of EMA for uncertainty quantification.
    #[serde(default = "default_beta")]
    pub beta2: f64,

    /// Coefficient of orthogonal regularization.
    #[serde(default = "default_orth_reg")]
    pub orth_reg_weight: f64,

    /// Total training steps (required for AdaLoRA).
    pub total_step: usize,
}

fn default_target_modules() -> Vec<String> {
    vec!["q_proj".into(), "v_proj".into()]
}

fn default_delta_t() -> usize {
    1
}

fn default_beta() -> f64 {
    0.85
}

fn default_orth_reg() -> f64 {
    0.5
}

impl Default for AdaLoraConfig {
    fn default() -> Self {
        Self {
            target_r: 8,
            init_r: 12,
            alpha: 16,
            dropout: 0.0,
            target_modules: default_target_modules(),
            tinit: 0,
            tfinal: 0,
            delta_t: default_delta_t(),
            beta1: default_beta(),
            beta2: default_beta(),
            orth_reg_weight: default_orth_reg(),
            total_step: 1000, // Must be set by user
        }
    }
}

impl AdapterConfig for AdaLoraConfig {
    fn validate(&self) -> Result<()> {
        if self.init_r == 0 {
            return Err(PeftError::InvalidConfig("init_r must be > 0".into()));
        }
        if self.target_r == 0 {
            return Err(PeftError::InvalidConfig("target_r must be > 0".into()));
        }
        if self.target_r > self.init_r {
            return Err(PeftError::InvalidConfig(
                "target_r must be <= init_r".into(),
            ));
        }
        if self.alpha == 0 {
            return Err(PeftError::InvalidConfig("alpha must be > 0".into()));
        }
        if !(0.0..=1.0).contains(&self.dropout) {
            return Err(PeftError::InvalidConfig(
                "dropout must be between 0 and 1".into(),
            ));
        }
        if self.total_step == 0 {
            return Err(PeftError::InvalidConfig("total_step must be > 0".into()));
        }
        if self.tinit >= self.total_step.saturating_sub(self.tfinal) {
            return Err(PeftError::InvalidConfig(
                "tinit must be < (total_step - tfinal) for budgeting phase".into(),
            ));
        }
        if !(0.0..=1.0).contains(&self.beta1) || !(0.0..=1.0).contains(&self.beta2) {
            return Err(PeftError::InvalidConfig(
                "beta1 and beta2 must be between 0 and 1".into(),
            ));
        }
        Ok(())
    }
}

/// AdaLoRA layer using SVD-based parameterization.
///
/// Uses `W = W0 + P * Λ * Q` where:
/// - P: Left singular vectors (out_features × r)
/// - Λ: Diagonal singular values (r)
/// - Q: Right singular vectors (r × in_features)
///
/// This allows for dynamic rank allocation by zeroing out singular values.
pub struct AdaLoraLayer {
    /// Left singular vectors: [out_features, init_r]
    lora_a: Tensor,
    /// Singular values: [init_r]
    lora_e: Tensor,
    /// Right singular vectors: [init_r, in_features]
    lora_b: Tensor,
    /// Scaling factor
    scaling: f64,
    /// Configuration
    config: AdaLoraConfig,
    /// Input dimension
    in_features: usize,
    /// Output dimension
    out_features: usize,
    /// Current rank (may be reduced during training)
    current_rank: usize,
    /// Mask for pruned singular values
    rank_mask: Tensor,
    /// Whether gradients are disabled
    frozen: bool,
}

impl AdaLoraLayer {
    /// Create a new AdaLoRA layer.
    ///
    /// # Arguments
    /// * `in_features` - Input dimension
    /// * `out_features` - Output dimension
    /// * `config` - AdaLoRA configuration
    /// * `device` - Device to create tensors on
    ///
    /// # Errors
    /// Returns error if configuration is invalid or tensor initialization fails.
    pub fn new(
        in_features: usize,
        out_features: usize,
        config: AdaLoraConfig,
        device: &Device,
    ) -> Result<Self> {
        config.validate()?;

        let scaling = config.alpha as f64 / config.init_r as f64;
        let dtype = DType::F32;

        // Initialize A (left singular vectors) with orthogonal-like initialization
        let std_a = (1.0 / out_features as f64).sqrt();
        let lora_a = Tensor::randn(0.0f32, std_a as f32, (out_features, config.init_r), device)?;

        // Initialize E (singular values) to small values
        let lora_e = Tensor::ones(config.init_r, dtype, device)?;
        let lora_e = lora_e.broadcast_mul(&Tensor::new(0.01f32, device)?)?;

        // Initialize B (right singular vectors) with orthogonal-like initialization
        let std_b = (1.0 / in_features as f64).sqrt();
        let lora_b = Tensor::randn(0.0f32, std_b as f32, (config.init_r, in_features), device)?;

        // Initialize rank mask to all ones (all ranks active)
        let rank_mask = Tensor::ones(config.init_r, dtype, device)?;

        let init_r = config.init_r;

        Ok(Self {
            lora_a,
            lora_e,
            lora_b,
            scaling,
            config,
            in_features,
            out_features,
            current_rank: init_r,
            rank_mask,
            frozen: false,
        })
    }

    /// Get the current active rank.
    #[must_use]
    pub fn current_rank(&self) -> usize {
        self.current_rank
    }

    /// Get the target rank.
    #[must_use]
    pub fn target_rank(&self) -> usize {
        self.config.target_r
    }

    /// Get the initial rank.
    #[must_use]
    pub fn init_rank(&self) -> usize {
        self.config.init_r
    }

    /// Get the scaling factor.
    #[must_use]
    pub fn scaling(&self) -> f64 {
        self.scaling
    }

    /// Update the rank mask based on importance scores.
    ///
    /// # Arguments
    /// * `importance_scores` - Importance score for each rank [init_r]
    /// * `budget` - Number of ranks to keep
    ///
    /// # Errors
    /// Returns error if tensor operations fail.
    pub fn update_rank_mask(&mut self, importance_scores: &Tensor, budget: usize) -> Result<()> {
        // Get the indices of top-k importance scores
        // For simplicity, we'll create a mask based on a threshold
        // In practice, this would involve sorting and selecting top-k

        if budget >= self.config.init_r {
            // Keep all ranks
            self.rank_mask =
                Tensor::ones(self.config.init_r, DType::F32, importance_scores.device())?;
            self.current_rank = self.config.init_r;
        } else if budget == 0 {
            // Zero out all ranks
            self.rank_mask =
                Tensor::zeros(self.config.init_r, DType::F32, importance_scores.device())?;
            self.current_rank = 0;
        } else {
            // Sort importance scores and keep top budget
            // Note: This is a simplified version - in practice would use argsort
            let scores = importance_scores.flatten_all()?;
            let mean_score = scores.mean_all()?;
            let mean: f32 = mean_score.to_scalar()?;

            // Simple threshold-based approach
            let threshold = Tensor::new(mean, importance_scores.device())?;
            let mask = importance_scores.ge(&threshold)?;
            self.rank_mask = mask.to_dtype(DType::F32)?;

            // Update current rank (count non-zero elements)
            let sum: f32 = self.rank_mask.sum_all()?.to_scalar()?;
            self.current_rank = sum as usize;
        }

        Ok(())
    }

    /// Compute the orthogonal regularization loss.
    ///
    /// Encourages P^T P ≈ I and Q Q^T ≈ I.
    ///
    /// # Errors
    /// Returns error if tensor operations fail.
    pub fn orthogonal_regularization(&self) -> Result<Tensor> {
        // P^T P - I
        let pta = self.lora_a.t()?.matmul(&self.lora_a)?;
        let eye_a = Tensor::eye(self.config.init_r, DType::F32, self.lora_a.device())?;
        let orth_loss_a = pta.broadcast_sub(&eye_a)?.sqr()?.sum_all()?;

        // Q Q^T - I
        let bbt = self.lora_b.matmul(&self.lora_b.t()?)?;
        let eye_b = Tensor::eye(self.config.init_r, DType::F32, self.lora_b.device())?;
        let orth_loss_b = bbt.broadcast_sub(&eye_b)?.sqr()?.sum_all()?;

        Ok(orth_loss_a.broadcast_add(&orth_loss_b)?)
    }

    /// Get the importance scores for rank allocation.
    ///
    /// The importance is based on the magnitude of singular values.
    ///
    /// # Errors
    /// Returns error if tensor operations fail.
    pub fn get_importance_scores(&self) -> Result<Tensor> {
        // Simple importance: absolute value of singular values
        Ok(self.lora_e.abs()?)
    }
}

impl Adapter for AdaLoraLayer {
    type Config = AdaLoraConfig;

    fn forward(&self, input: &Tensor, base_output: Option<&Tensor>) -> Result<Tensor> {
        // AdaLoRA forward: x @ B^T @ diag(E * mask) @ A^T * scaling
        // Input shape: [batch, seq, in_features]
        // B shape: [init_r, in_features], B^T shape: [in_features, init_r]
        // A shape: [out_features, init_r], A^T shape: [init_r, out_features]

        // For batched matmul, we need to handle the 3D tensor
        // Use the last dimension for matmul
        let input_dims = input.dims();

        // First: input @ B^T -> [batch, seq, init_r]
        // Reshape input to [batch*seq, in_features] for matmul
        let batch_seq = input_dims[0] * input_dims[1];
        let input_2d = input.reshape((batch_seq, self.in_features))?;

        // Compute input_2d @ B^T = [batch*seq, in_features] @ [in_features, init_r] = [batch*seq, init_r]
        let out = input_2d.matmul(&self.lora_b.t()?)?;

        // Apply singular values with mask: [batch*seq, init_r] * [init_r]
        let masked_e = self.lora_e.broadcast_mul(&self.rank_mask)?;
        let masked_e = masked_e.reshape((1, self.config.init_r))?;
        let out = out.broadcast_mul(&masked_e)?;

        // Then: @ A^T -> [batch*seq, out_features]
        // A^T shape: [init_r, out_features]
        let out = out.matmul(&self.lora_a.t()?)?;

        // Reshape back to [batch, seq, out_features]
        let out = out.reshape((input_dims[0], input_dims[1], self.out_features))?;

        // Apply scaling
        let scaling = Tensor::new(self.scaling as f32, out.device())?;
        let out = out.broadcast_mul(&scaling)?;

        // Add to base output if provided
        match base_output {
            Some(base) => Ok(base.broadcast_add(&out)?),
            None => Ok(out),
        }
    }

    fn num_parameters(&self) -> usize {
        // A: out_features × init_r
        // E: init_r
        // B: init_r × in_features
        self.out_features * self.config.init_r
            + self.config.init_r
            + self.config.init_r * self.in_features
    }

    fn config(&self) -> &Self::Config {
        &self.config
    }
}

impl Mergeable for AdaLoraLayer {
    fn merge(&self, base_weight: &Tensor) -> Result<Tensor> {
        // ΔW = A @ diag(E * mask) @ B * scaling
        // Apply mask to singular values
        let masked_e = self.lora_e.broadcast_mul(&self.rank_mask)?;

        // Compute A @ diag(E) = A * E (broadcast along columns)
        let masked_e_col = masked_e.reshape((self.config.init_r, 1))?;
        let ae = self.lora_a.broadcast_mul(&masked_e_col.t()?)?;

        // Then @ B
        let delta_w = ae.matmul(&self.lora_b)?;

        // Apply scaling
        let scaling = Tensor::new(self.scaling as f32, delta_w.device())?;
        let delta_w = delta_w.broadcast_mul(&scaling)?;

        Ok(base_weight.broadcast_add(&delta_w)?)
    }

    fn unmerge(&self, merged_weight: &Tensor) -> Result<Tensor> {
        let masked_e = self.lora_e.broadcast_mul(&self.rank_mask)?;

        let masked_e_col = masked_e.reshape((self.config.init_r, 1))?;
        let ae = self.lora_a.broadcast_mul(&masked_e_col.t()?)?;
        let delta_w = ae.matmul(&self.lora_b)?;

        let scaling = Tensor::new(self.scaling as f32, delta_w.device())?;
        let delta_w = delta_w.broadcast_mul(&scaling)?;

        Ok(merged_weight.broadcast_sub(&delta_w)?)
    }
}

impl Trainable for AdaLoraLayer {
    fn register_parameters(&self, _var_map: &mut VarMap, _prefix: &str) -> Result<()> {
        // Note: In the current design, tensors are created directly.
        // For full training support, tensors should be created via VarBuilder
        // during construction, which automatically registers them.
        // This is a simplified implementation suitable for inference.
        Ok(())
    }

    fn freeze(&mut self) {
        self.frozen = true;
    }

    fn unfreeze(&mut self) {
        self.frozen = false;
    }

    fn is_frozen(&self) -> bool {
        self.frozen
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adalora_config_default() {
        let config = AdaLoraConfig::default();
        assert_eq!(config.target_r, 8);
        assert_eq!(config.init_r, 12);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_adalora_config_invalid_rank() {
        let config = AdaLoraConfig {
            target_r: 16,
            init_r: 8, // target > init is invalid
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_adalora_config_invalid_schedule() {
        let config = AdaLoraConfig {
            tinit: 500,
            tfinal: 600,
            total_step: 1000, // tinit >= total_step - tfinal
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_adalora_layer_creation() {
        let config = AdaLoraConfig::default();
        let device = Device::Cpu;
        let layer = AdaLoraLayer::new(768, 768, config, &device);
        assert!(layer.is_ok());

        let layer = layer.unwrap();
        assert_eq!(layer.init_rank(), 12);
        assert_eq!(layer.target_rank(), 8);
        assert_eq!(layer.current_rank(), 12);
    }

    #[test]
    fn test_adalora_forward_shape() {
        let config = AdaLoraConfig::default();
        let device = Device::Cpu;
        let layer = AdaLoraLayer::new(768, 768, config, &device).unwrap();

        let input = Tensor::zeros(&[1, 10, 768], DType::F32, &device).unwrap();
        let output = layer.forward(&input, None).unwrap();

        assert_eq!(output.shape().dims(), &[1, 10, 768]);
    }

    #[test]
    fn test_adalora_num_parameters() {
        let config = AdaLoraConfig {
            init_r: 12,
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer = AdaLoraLayer::new(768, 768, config, &device).unwrap();

        // A: 768 × 12 = 9216
        // E: 12
        // B: 12 × 768 = 9216
        // Total: 18444
        assert_eq!(layer.num_parameters(), 768 * 12 + 12 + 12 * 768);
    }

    #[test]
    fn test_adalora_importance_scores() {
        let config = AdaLoraConfig::default();
        let device = Device::Cpu;
        let layer = AdaLoraLayer::new(768, 768, config, &device).unwrap();

        let scores = layer.get_importance_scores().unwrap();
        assert_eq!(scores.dims(), &[12]);
    }

    #[test]
    fn test_adalora_orthogonal_regularization() {
        let config = AdaLoraConfig::default();
        let device = Device::Cpu;
        let layer = AdaLoraLayer::new(64, 64, config, &device).unwrap();

        let orth_loss = layer.orthogonal_regularization().unwrap();
        // Should be a scalar tensor (0-dimensional)
        assert!(orth_loss.dims().is_empty());
    }
}
