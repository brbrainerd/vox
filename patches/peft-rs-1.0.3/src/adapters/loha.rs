//! `LoHa` (Low-Rank Hadamard Product) implementation.
//!
//! `LoHa` uses the Hadamard (element-wise) product of two low-rank matrices
//! for more expressive weight updates: `ΔW = (A1 ⊗ B1) ⊙ (A2 ⊗ B2)`
//!
//! Reference: <https://arxiv.org/abs/2108.06098> (`LyCORIS`)

#![allow(clippy::doc_markdown)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_precision_loss)]

use candle_core::{Device, Tensor};
use candle_nn::VarMap;
use serde::{Deserialize, Serialize};

use crate::error::{PeftError, Result};
use crate::traits::{Adapter, AdapterConfig, Mergeable, Trainable};

/// Configuration for LoHa adapters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoHaConfig {
    /// Rank of the first low-rank decomposition.
    pub r: usize,

    /// Scaling factor (typically `alpha / r`).
    pub alpha: usize,

    /// Target modules to apply LoHa to.
    #[serde(default = "default_target_modules")]
    pub target_modules: Vec<String>,

    /// Whether to use effective convolution for conv layers.
    #[serde(default)]
    pub use_effective_conv2d: bool,
}

fn default_target_modules() -> Vec<String> {
    vec!["q_proj".into(), "v_proj".into()]
}

impl Default for LoHaConfig {
    fn default() -> Self {
        Self {
            r: 8,
            alpha: 16,
            target_modules: default_target_modules(),
            use_effective_conv2d: false,
        }
    }
}

impl AdapterConfig for LoHaConfig {
    fn validate(&self) -> Result<()> {
        if self.r == 0 {
            return Err(PeftError::InvalidConfig("rank must be > 0".into()));
        }
        if self.alpha == 0 {
            return Err(PeftError::InvalidConfig("alpha must be > 0".into()));
        }
        Ok(())
    }
}

/// LoHa layer implementing Low-Rank Hadamard Product adaptation.
///
/// Computes: `ΔW = (A1 @ B1) ⊙ (A2 @ B2) * scaling`
///
/// Where:
/// - A1, A2: [out_features, r]
/// - B1, B2: [r, in_features]
/// - ⊙ is element-wise (Hadamard) product
pub struct LoHaLayer {
    /// First decomposition: A1 [out_features, r]
    hada_w1_a: Tensor,
    /// First decomposition: B1 [r, in_features]
    hada_w1_b: Tensor,
    /// Second decomposition: A2 [out_features, r]
    hada_w2_a: Tensor,
    /// Second decomposition: B2 [r, in_features]
    hada_w2_b: Tensor,
    /// Scaling factor = alpha / r
    scaling: f64,
    /// Configuration
    config: LoHaConfig,
    /// Input dimension
    in_features: usize,
    /// Output dimension
    out_features: usize,
    /// Whether gradients are disabled
    frozen: bool,
}

impl LoHaLayer {
    /// Create a new LoHa layer.
    ///
    /// # Arguments
    /// * `in_features` - Input dimension
    /// * `out_features` - Output dimension
    /// * `config` - LoHa configuration
    /// * `device` - Device to create tensors on
    ///
    /// # Errors
    /// Returns error if configuration is invalid or tensor initialization fails.
    pub fn new(
        in_features: usize,
        out_features: usize,
        config: LoHaConfig,
        device: &Device,
    ) -> Result<Self> {
        config.validate()?;

        let scaling = config.alpha as f64 / config.r as f64;

        // Initialize with Kaiming-like initialization
        let std = (1.0 / config.r as f64).sqrt() as f32;

        // First low-rank decomposition
        let hada_w1_a = Tensor::randn(0.0f32, std, (out_features, config.r), device)?;
        let hada_w1_b = Tensor::randn(0.0f32, std, (config.r, in_features), device)?;

        // Second low-rank decomposition
        let hada_w2_a = Tensor::randn(0.0f32, std, (out_features, config.r), device)?;
        let hada_w2_b = Tensor::randn(0.0f32, std, (config.r, in_features), device)?;

        Ok(Self {
            hada_w1_a,
            hada_w1_b,
            hada_w2_a,
            hada_w2_b,
            scaling,
            config,
            in_features,
            out_features,
            frozen: false,
        })
    }

    /// Get the scaling factor.
    #[must_use]
    pub fn scaling(&self) -> f64 {
        self.scaling
    }

    /// Get the rank.
    #[must_use]
    pub fn rank(&self) -> usize {
        self.config.r
    }

    /// Compute the weight delta: (A1 @ B1) ⊙ (A2 @ B2)
    fn compute_delta_w(&self) -> Result<Tensor> {
        // Compute first term: A1 @ B1 -> [out_features, in_features]
        let term1 = self.hada_w1_a.matmul(&self.hada_w1_b)?;

        // Compute second term: A2 @ B2 -> [out_features, in_features]
        let term2 = self.hada_w2_a.matmul(&self.hada_w2_b)?;

        // Hadamard (element-wise) product
        Ok(term1.mul(&term2)?)
    }
}

impl Adapter for LoHaLayer {
    type Config = LoHaConfig;

    fn forward(&self, input: &Tensor, base_output: Option<&Tensor>) -> Result<Tensor> {
        // Compute delta weight
        let delta_w = self.compute_delta_w()?;

        // Apply scaling
        let scaling = Tensor::new(self.scaling as f32, delta_w.device())?;
        let delta_w = delta_w.broadcast_mul(&scaling)?;

        // Compute: input @ delta_w^T
        let input_dims = input.dims();
        let batch_seq = input_dims[0] * input_dims[1];
        let input_2d = input.reshape((batch_seq, self.in_features))?;

        let loha_out = input_2d.matmul(&delta_w.t()?)?;
        let loha_out = loha_out.reshape((input_dims[0], input_dims[1], self.out_features))?;

        // Add to base output if provided
        match base_output {
            Some(base) => Ok(base.broadcast_add(&loha_out)?),
            None => Ok(loha_out),
        }
    }

    fn num_parameters(&self) -> usize {
        // 4 matrices: 2 * (out_features * r + r * in_features)
        2 * (self.out_features * self.config.r + self.config.r * self.in_features)
    }

    fn config(&self) -> &Self::Config {
        &self.config
    }
}

impl Mergeable for LoHaLayer {
    fn merge(&self, base_weight: &Tensor) -> Result<Tensor> {
        let delta_w = self.compute_delta_w()?;
        let scaling = Tensor::new(self.scaling as f32, delta_w.device())?;
        let delta_w = delta_w.broadcast_mul(&scaling)?;

        Ok(base_weight.broadcast_add(&delta_w)?)
    }

    fn unmerge(&self, merged_weight: &Tensor) -> Result<Tensor> {
        let delta_w = self.compute_delta_w()?;
        let scaling = Tensor::new(self.scaling as f32, delta_w.device())?;
        let delta_w = delta_w.broadcast_mul(&scaling)?;

        Ok(merged_weight.broadcast_sub(&delta_w)?)
    }
}

impl Trainable for LoHaLayer {
    fn register_parameters(&self, _var_map: &mut VarMap, _prefix: &str) -> Result<()> {
        // Note: In the current design, tensors are created directly.
        // For full training support, tensors should be created via VarBuilder
        // during construction, which automatically registers them.
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
    use candle_core::DType;

    #[test]
    fn test_loha_config_default() {
        let config = LoHaConfig::default();
        assert_eq!(config.r, 8);
        assert_eq!(config.alpha, 16);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_loha_config_invalid_rank() {
        let config = LoHaConfig {
            r: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_loha_config_invalid_alpha() {
        let config = LoHaConfig {
            alpha: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_loha_layer_creation() {
        let config = LoHaConfig::default();
        let device = Device::Cpu;
        let layer = LoHaLayer::new(768, 768, config, &device);
        assert!(layer.is_ok());
    }

    #[test]
    fn test_loha_forward_shape() {
        let config = LoHaConfig::default();
        let device = Device::Cpu;
        let layer = LoHaLayer::new(768, 768, config, &device).unwrap();

        let input = Tensor::zeros(&[1, 10, 768], DType::F32, &device).unwrap();
        let output = layer.forward(&input, None).unwrap();

        assert_eq!(output.shape().dims(), &[1, 10, 768]);
    }

    #[test]
    fn test_loha_forward_with_base_output() {
        let config = LoHaConfig::default();
        let device = Device::Cpu;
        let layer = LoHaLayer::new(768, 768, config, &device).unwrap();

        let input = Tensor::zeros(&[1, 10, 768], DType::F32, &device).unwrap();
        let base_output = Tensor::ones(&[1, 10, 768], DType::F32, &device).unwrap();
        let output = layer.forward(&input, Some(&base_output)).unwrap();

        assert_eq!(output.shape().dims(), &[1, 10, 768]);
    }

    #[test]
    fn test_loha_num_parameters() {
        let config = LoHaConfig {
            r: 8,
            alpha: 16,
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer = LoHaLayer::new(768, 768, config, &device).unwrap();

        // 2 * (out * r + r * in) = 2 * (768 * 8 + 8 * 768) = 2 * 12288 = 24576
        assert_eq!(layer.num_parameters(), 24576);
    }

    #[test]
    fn test_loha_merge_unmerge() {
        let config = LoHaConfig::default();
        let device = Device::Cpu;
        let layer = LoHaLayer::new(64, 64, config, &device).unwrap();

        let base_weight = Tensor::randn(0.0f32, 0.02, (64, 64), &device).unwrap();
        let merged = layer.merge(&base_weight).unwrap();
        let unmerged = layer.unmerge(&merged).unwrap();

        // Unmerged should be close to original
        let diff = unmerged.broadcast_sub(&base_weight).unwrap();
        let max_diff: f32 = diff
            .abs()
            .unwrap()
            .max(0)
            .unwrap()
            .max(0)
            .unwrap()
            .to_scalar()
            .unwrap();
        assert!(max_diff < 1e-5);
    }

    #[test]
    fn test_loha_freeze_unfreeze() {
        let config = LoHaConfig::default();
        let device = Device::Cpu;
        let mut layer = LoHaLayer::new(768, 768, config, &device).unwrap();

        assert!(!layer.is_frozen());
        layer.freeze();
        assert!(layer.is_frozen());
        layer.unfreeze();
        assert!(!layer.is_frozen());
    }
}
