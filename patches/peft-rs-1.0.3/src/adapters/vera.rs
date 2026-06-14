//! `VeRA` (Vector-based Random Matrix Adaptation) implementation.
//!
//! `VeRA` uses frozen random matrices with trainable scaling vectors for
//! ultra-efficient adaptation. It achieves similar performance to `LoRA`
//! with significantly fewer trainable parameters.
//!
//! Reference: <https://arxiv.org/abs/2310.11454>

use candle_core::{Device, Tensor};
use candle_nn::VarMap;
use serde::{Deserialize, Serialize};

use crate::error::{PeftError, Result};
use crate::traits::{Adapter, AdapterConfig, Mergeable, Trainable};

/// Configuration for `VeRA` adapters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VeraConfig {
    /// Rank of the random projection matrices.
    pub r: usize,

    /// Initial value for the scaling vector d.
    #[serde(default = "default_d_initial")]
    pub d_initial: f64,

    /// Seed for random number generation (for reproducible projections).
    #[serde(default)]
    pub projection_prng_key: u64,

    /// Whether to save the projection matrices (for exact reproducibility).
    #[serde(default)]
    pub save_projection: bool,

    /// Target modules to apply `VeRA` to.
    #[serde(default = "default_target_modules")]
    pub target_modules: Vec<String>,
}

fn default_d_initial() -> f64 {
    0.1
}

fn default_target_modules() -> Vec<String> {
    vec!["q_proj".into(), "v_proj".into()]
}

impl Default for VeraConfig {
    fn default() -> Self {
        Self {
            r: 256,
            d_initial: default_d_initial(),
            projection_prng_key: 0,
            save_projection: false,
            target_modules: default_target_modules(),
        }
    }
}

impl AdapterConfig for VeraConfig {
    fn validate(&self) -> Result<()> {
        if self.r == 0 {
            return Err(PeftError::InvalidConfig("rank must be > 0".into()));
        }
        Ok(())
    }
}

/// `VeRA` layer implementing Vector-based Random Matrix Adaptation.
///
/// Uses frozen random matrices A and B with trainable scaling vectors:
/// `ΔW = B @ diag(d) @ A`
///
/// Where:
/// - A: Frozen random matrix [r, `in_features`] (Kaiming initialization)
/// - B: Frozen random matrix [`out_features`, r] (zero initialization or small random)
/// - d: Trainable scaling vector [r]
/// - b: Optional trainable bias vector [`out_features`]
pub struct VeraLayer {
    /// Frozen random projection A: [r, `in_features`]
    vera_a: Tensor,
    /// Frozen random projection B: [`out_features`, r]
    vera_b: Tensor,
    /// Trainable scaling vector d: [r]
    vera_d: Tensor,
    /// Optional trainable bias b: [`out_features`]
    vera_b_bias: Option<Tensor>,
    /// Configuration
    config: VeraConfig,
    /// Input dimension
    in_features: usize,
    /// Output dimension
    out_features: usize,
    /// Whether gradients are disabled
    frozen: bool,
}

impl VeraLayer {
    /// Create a new `VeRA` layer.
    ///
    /// # Arguments
    /// * `in_features` - Input dimension
    /// * `out_features` - Output dimension
    /// * `config` - `VeRA` configuration
    /// * `device` - Device to create tensors on
    ///
    /// # Errors
    ///
    /// Returns an error if configuration validation fails or layer construction fails.
    pub fn new(
        in_features: usize,
        out_features: usize,
        config: VeraConfig,
        device: &Device,
    ) -> Result<Self> {
        config.validate()?;

        // Initialize frozen random projection A with Kaiming uniform
        #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
        let std_a = (1.0 / in_features as f64).sqrt() as f32;
        let vera_a = Tensor::randn(0.0f32, std_a, (config.r, in_features), device)?;

        // Initialize frozen random projection B with zeros (or small random)
        // In the original paper, B is initialized to zeros for a clean start
        let vera_b = Tensor::zeros((out_features, config.r), candle_core::DType::F32, device)?;

        // Initialize trainable scaling vector d
        #[allow(clippy::cast_possible_truncation)]
        let vera_d = Tensor::full(config.d_initial as f32, config.r, device)?;

        Ok(Self {
            vera_a,
            vera_b,
            vera_d,
            vera_b_bias: None,
            config,
            in_features,
            out_features,
            frozen: false,
        })
    }

    /// Create a new `VeRA` layer with trainable bias.
    ///
    /// # Arguments
    /// * `in_features` - Input dimension
    /// * `out_features` - Output dimension
    /// * `config` - `VeRA` configuration
    /// * `device` - Device to create tensors on
    ///
    /// # Errors
    ///
    /// Returns an error if configuration validation fails or layer construction fails.
    pub fn new_with_bias(
        in_features: usize,
        out_features: usize,
        config: VeraConfig,
        device: &Device,
    ) -> Result<Self> {
        let mut layer = Self::new(in_features, out_features, config, device)?;
        layer.vera_b_bias = Some(Tensor::zeros(
            out_features,
            candle_core::DType::F32,
            device,
        )?);
        Ok(layer)
    }

    /// Get the scaling vector d.
    #[must_use]
    pub fn scaling_vector(&self) -> &Tensor {
        &self.vera_d
    }

    /// Get the rank.
    #[must_use]
    pub fn rank(&self) -> usize {
        self.config.r
    }

    /// Compute the weight delta: B @ diag(d) @ A
    fn compute_delta_w(&self) -> Result<Tensor> {
        // diag(d) @ A: scale each row of A by corresponding element of d
        // d: [r], A: [r, in_features]
        // Result: [r, in_features]
        let d_col = self.vera_d.reshape((self.config.r, 1))?;
        let da = self.vera_a.broadcast_mul(&d_col)?;

        // B @ (diag(d) @ A)
        // B: [out_features, r], da: [r, in_features]
        // Result: [out_features, in_features]
        Ok(self.vera_b.matmul(&da)?)
    }
}

impl Adapter for VeraLayer {
    type Config = VeraConfig;

    fn forward(&self, input: &Tensor, base_output: Option<&Tensor>) -> Result<Tensor> {
        // Compute delta weight
        let delta_w = self.compute_delta_w()?;

        // Compute: input @ delta_w^T
        let input_dims = input.dims();
        let batch_seq = input_dims[0] * input_dims[1];
        let input_2d = input.reshape((batch_seq, self.in_features))?;

        let mut vera_out = input_2d.matmul(&delta_w.t()?)?;

        // Add bias if present
        if let Some(bias) = &self.vera_b_bias {
            let bias_expanded = bias.reshape((1, self.out_features))?;
            vera_out = vera_out.broadcast_add(&bias_expanded)?;
        }

        let vera_out = vera_out.reshape((input_dims[0], input_dims[1], self.out_features))?;

        // Add to base output if provided
        match base_output {
            Some(base) => Ok(base.broadcast_add(&vera_out)?),
            None => Ok(vera_out),
        }
    }

    fn num_parameters(&self) -> usize {
        // Only the scaling vector d is trainable
        // Optionally, bias b is also trainable
        let mut params = self.config.r;
        if self.vera_b_bias.is_some() {
            params += self.out_features;
        }
        params
    }

    fn config(&self) -> &Self::Config {
        &self.config
    }
}

impl Mergeable for VeraLayer {
    fn merge(&self, base_weight: &Tensor) -> Result<Tensor> {
        let delta_w = self.compute_delta_w()?;
        Ok(base_weight.broadcast_add(&delta_w)?)
    }

    fn unmerge(&self, merged_weight: &Tensor) -> Result<Tensor> {
        let delta_w = self.compute_delta_w()?;
        Ok(merged_weight.broadcast_sub(&delta_w)?)
    }
}

impl Trainable for VeraLayer {
    fn register_parameters(&self, _var_map: &mut VarMap, _prefix: &str) -> Result<()> {
        // Note: In the current design, tensors are created directly.
        // For full training support, only vera_d (and optionally vera_b_bias)
        // should be registered as trainable. vera_a and vera_b are frozen.
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
    fn test_vera_config_default() {
        let config = VeraConfig::default();
        assert_eq!(config.r, 256);
        assert!((config.d_initial - 0.1).abs() < 1e-6);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_vera_config_invalid_rank() {
        let config = VeraConfig {
            r: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_vera_layer_creation() {
        let config = VeraConfig {
            r: 64,
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer = VeraLayer::new(768, 768, config, &device);
        assert!(layer.is_ok());
    }

    #[test]
    fn test_vera_layer_with_bias() {
        let config = VeraConfig {
            r: 64,
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer = VeraLayer::new_with_bias(768, 768, config, &device);
        assert!(layer.is_ok());

        let layer = layer.unwrap();
        assert!(layer.vera_b_bias.is_some());
    }

    #[test]
    fn test_vera_forward_shape() {
        let config = VeraConfig {
            r: 64,
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer = VeraLayer::new(768, 768, config, &device).unwrap();

        let input = Tensor::zeros(&[1, 10, 768], DType::F32, &device).unwrap();
        let output = layer.forward(&input, None).unwrap();

        assert_eq!(output.shape().dims(), &[1, 10, 768]);
    }

    #[test]
    fn test_vera_forward_with_base_output() {
        let config = VeraConfig {
            r: 64,
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer = VeraLayer::new(768, 768, config, &device).unwrap();

        let input = Tensor::zeros(&[1, 10, 768], DType::F32, &device).unwrap();
        let base_output = Tensor::ones(&[1, 10, 768], DType::F32, &device).unwrap();
        let output = layer.forward(&input, Some(&base_output)).unwrap();

        assert_eq!(output.shape().dims(), &[1, 10, 768]);
    }

    #[test]
    fn test_vera_num_parameters() {
        let config = VeraConfig {
            r: 64,
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer = VeraLayer::new(768, 768, config, &device).unwrap();

        // Only d vector is trainable: 64 parameters
        assert_eq!(layer.num_parameters(), 64);
    }

    #[test]
    fn test_vera_num_parameters_with_bias() {
        let config = VeraConfig {
            r: 64,
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer = VeraLayer::new_with_bias(768, 768, config, &device).unwrap();

        // d vector + bias: 64 + 768 = 832
        assert_eq!(layer.num_parameters(), 64 + 768);
    }

    #[test]
    fn test_vera_merge_unmerge() {
        let config = VeraConfig {
            r: 32,
            d_initial: 0.01,
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer = VeraLayer::new(64, 64, config, &device).unwrap();

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
    fn test_vera_freeze_unfreeze() {
        let config = VeraConfig::default();
        let device = Device::Cpu;
        let mut layer = VeraLayer::new(768, 768, config, &device).unwrap();

        assert!(!layer.is_frozen());
        layer.freeze();
        assert!(layer.is_frozen());
        layer.unfreeze();
        assert!(!layer.is_frozen());
    }

    #[test]
    fn test_vera_ultra_efficient() {
        // VeRA should have far fewer parameters than LoRA for same rank
        let config = VeraConfig {
            r: 64,
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer = VeraLayer::new(768, 768, config, &device).unwrap();

        // VeRA: only 64 trainable params (the d vector)
        // LoRA with r=64: 64 * 768 + 64 * 768 = 98,304 params
        assert_eq!(layer.num_parameters(), 64);

        // That's ~1500x fewer parameters than equivalent LoRA!
        let lora_equivalent_params = 64 * (768 + 768);
        assert!(layer.num_parameters() < lora_equivalent_params / 1000);
    }
}
