//! `LoKr` (Low-Rank Kronecker Product) implementation.
//!
//! `LoKr` uses Kronecker product decomposition for efficient weight updates.
//! The weight matrix is factorized as: `ΔW = kron(A, B)` where the Kronecker
//! product allows for structured, parameter-efficient representations.
//!
//! Reference: <https://arxiv.org/abs/2108.06098> (`LyCORIS`)

#![allow(clippy::doc_markdown)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]

use candle_core::{Device, Tensor};
use candle_nn::VarMap;
use serde::{Deserialize, Serialize};

use crate::error::{PeftError, Result};
use crate::traits::{Adapter, AdapterConfig, Mergeable, Trainable};

/// Configuration for LoKr adapters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoKrConfig {
    /// Rank for the decomposition (used for one factor).
    pub r: usize,

    /// Scaling factor.
    pub alpha: usize,

    /// Factor dimension (splits the weight into factor x remaining).
    /// If None, uses automatic factorization.
    #[serde(default)]
    pub factor: Option<usize>,

    /// Decomposition type for the Kronecker factors.
    #[serde(default)]
    pub decompose_both: bool,

    /// Target modules to apply LoKr to.
    #[serde(default = "default_target_modules")]
    pub target_modules: Vec<String>,
}

fn default_target_modules() -> Vec<String> {
    vec!["q_proj".into(), "v_proj".into()]
}

impl Default for LoKrConfig {
    fn default() -> Self {
        Self {
            r: 8,
            alpha: 16,
            factor: None,
            decompose_both: false,
            target_modules: default_target_modules(),
        }
    }
}

impl AdapterConfig for LoKrConfig {
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

/// LoKr layer implementing Low-Rank Kronecker Product adaptation.
///
/// Uses a simplified Kronecker-like decomposition where the weight update
/// is computed as the outer product structure:
/// `ΔW = (w1 ⊗ w2) @ (w1_b ⊗ w2_b)^T` approximated via low-rank factors.
///
/// For simplicity, this implementation uses a factored approach:
/// - `lokr_w1`: First Kronecker factor [factor_out, factor_in]
/// - `lokr_w2_a`, `lokr_w2_b`: Low-rank decomposition of second factor
pub struct LoKrLayer {
    /// First Kronecker factor: [factor_out, factor_in]
    lokr_w1: Tensor,
    /// Second factor A (low-rank): [remaining_out, r]
    lokr_w2_a: Tensor,
    /// Second factor B (low-rank): [r, remaining_in]
    lokr_w2_b: Tensor,
    /// Scaling factor = alpha / r
    scaling: f64,
    /// Configuration
    config: LoKrConfig,
    /// Input dimension
    in_features: usize,
    /// Output dimension
    out_features: usize,
    /// Factor for output dimension
    factor_out: usize,
    /// Factor for input dimension
    factor_in: usize,
    /// Whether gradients are disabled
    frozen: bool,
}

impl LoKrLayer {
    /// Create a new LoKr layer.
    ///
    /// # Arguments
    /// * `in_features` - Input dimension
    /// * `out_features` - Output dimension
    /// * `config` - LoKr configuration
    /// * `device` - Device to create tensors on
    ///
    /// # Errors
    /// Returns error if configuration is invalid or tensor initialization fails.
    pub fn new(
        in_features: usize,
        out_features: usize,
        config: LoKrConfig,
        device: &Device,
    ) -> Result<Self> {
        config.validate()?;

        let scaling = config.alpha as f64 / config.r as f64;

        // Determine factorization
        // For a weight [out, in], we factorize as kron([f_out, f_in], [r_out, r_in])
        // where out = f_out * r_out and in = f_in * r_in
        let factor = config.factor.unwrap_or_else(|| {
            // Find a reasonable factor (try to find a divisor close to sqrt)
            let target = (out_features as f64).sqrt() as usize;
            for f in (1..=target).rev() {
                if out_features.is_multiple_of(f) && in_features.is_multiple_of(f) {
                    return f;
                }
            }
            1
        });

        let factor_out = factor.min(out_features);
        let factor_in = factor.min(in_features);
        let remaining_out = out_features / factor_out;
        let remaining_in = in_features / factor_in;

        // Initialize weights
        let std = (1.0 / config.r as f64).sqrt() as f32;

        // First Kronecker factor (full matrix)
        let lokr_w1 = Tensor::randn(0.0f32, std, (factor_out, factor_in), device)?;

        // Second factor as low-rank: A @ B
        let lokr_w2_a = Tensor::randn(0.0f32, std, (remaining_out, config.r), device)?;
        let lokr_w2_b = Tensor::randn(0.0f32, std, (config.r, remaining_in), device)?;

        Ok(Self {
            lokr_w1,
            lokr_w2_a,
            lokr_w2_b,
            scaling,
            config,
            in_features,
            out_features,
            factor_out,
            factor_in,
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

    /// Compute the Kronecker product of two 2D tensors.
    /// kron(A, B) where A is [m, n] and B is [p, q] produces [m*p, n*q]
    #[allow(clippy::many_single_char_names)]
    fn kronecker_product(a: &Tensor, b: &Tensor) -> Result<Tensor> {
        let a_shape = a.dims();
        let b_shape = b.dims();

        let m = a_shape[0];
        let n = a_shape[1];
        let p = b_shape[0];
        let q = b_shape[1];

        // Result shape: [m*p, n*q]
        let mut result_data = Vec::with_capacity(m * p * n * q);

        // Get data as vectors
        let a_data: Vec<f32> = a.flatten_all()?.to_vec1()?;
        let b_data: Vec<f32> = b.flatten_all()?.to_vec1()?;

        // Compute Kronecker product
        for i in 0..m {
            for k in 0..p {
                for j in 0..n {
                    for l in 0..q {
                        let a_val = a_data[i * n + j];
                        let b_val = b_data[k * q + l];
                        result_data.push(a_val * b_val);
                    }
                }
            }
        }

        Ok(Tensor::from_vec(result_data, (m * p, n * q), a.device())?)
    }

    /// Compute the weight delta using Kronecker product.
    fn compute_delta_w(&self) -> Result<Tensor> {
        // Compute the second factor: w2_a @ w2_b
        let w2 = self.lokr_w2_a.matmul(&self.lokr_w2_b)?;

        // Compute Kronecker product: kron(w1, w2)
        Self::kronecker_product(&self.lokr_w1, &w2)
    }
}

impl Adapter for LoKrLayer {
    type Config = LoKrConfig;

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

        let lokr_out = input_2d.matmul(&delta_w.t()?)?;
        let lokr_out = lokr_out.reshape((input_dims[0], input_dims[1], self.out_features))?;

        // Add to base output if provided
        match base_output {
            Some(base) => Ok(base.broadcast_add(&lokr_out)?),
            None => Ok(lokr_out),
        }
    }

    fn num_parameters(&self) -> usize {
        let remaining_out = self.out_features / self.factor_out;
        let remaining_in = self.in_features / self.factor_in;

        // w1: factor_out * factor_in
        // w2_a: remaining_out * r
        // w2_b: r * remaining_in
        self.factor_out * self.factor_in
            + remaining_out * self.config.r
            + self.config.r * remaining_in
    }

    fn config(&self) -> &Self::Config {
        &self.config
    }
}

impl Mergeable for LoKrLayer {
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

impl Trainable for LoKrLayer {
    fn register_parameters(&self, _var_map: &mut VarMap, _prefix: &str) -> Result<()> {
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
    fn test_lokr_config_default() {
        let config = LoKrConfig::default();
        assert_eq!(config.r, 8);
        assert_eq!(config.alpha, 16);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_lokr_config_invalid_rank() {
        let config = LoKrConfig {
            r: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_lokr_layer_creation() {
        let config = LoKrConfig::default();
        let device = Device::Cpu;
        // Use dimensions that are easily factorizable
        let layer = LoKrLayer::new(64, 64, config, &device);
        assert!(layer.is_ok());
    }

    #[test]
    fn test_lokr_layer_with_factor() {
        let config = LoKrConfig {
            factor: Some(8),
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer = LoKrLayer::new(64, 64, config, &device);
        assert!(layer.is_ok());

        let layer = layer.unwrap();
        assert_eq!(layer.factor_out, 8);
        assert_eq!(layer.factor_in, 8);
    }

    #[test]
    fn test_lokr_forward_shape() {
        let config = LoKrConfig {
            factor: Some(8),
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer = LoKrLayer::new(64, 64, config, &device).unwrap();

        let input = Tensor::zeros(&[1, 10, 64], DType::F32, &device).unwrap();
        let output = layer.forward(&input, None).unwrap();

        assert_eq!(output.shape().dims(), &[1, 10, 64]);
    }

    #[test]
    fn test_lokr_forward_with_base_output() {
        let config = LoKrConfig {
            factor: Some(8),
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer = LoKrLayer::new(64, 64, config, &device).unwrap();

        let input = Tensor::zeros(&[1, 10, 64], DType::F32, &device).unwrap();
        let base_output = Tensor::ones(&[1, 10, 64], DType::F32, &device).unwrap();
        let output = layer.forward(&input, Some(&base_output)).unwrap();

        assert_eq!(output.shape().dims(), &[1, 10, 64]);
    }

    #[test]
    fn test_lokr_num_parameters() {
        let config = LoKrConfig {
            r: 4,
            factor: Some(8),
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer = LoKrLayer::new(64, 64, config, &device).unwrap();

        // w1: 8 * 8 = 64
        // remaining: 64/8 = 8
        // w2_a: 8 * 4 = 32
        // w2_b: 4 * 8 = 32
        // Total: 64 + 32 + 32 = 128
        assert_eq!(layer.num_parameters(), 128);
    }

    #[test]
    fn test_lokr_merge_unmerge() {
        let config = LoKrConfig {
            factor: Some(8),
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer = LoKrLayer::new(64, 64, config, &device).unwrap();

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
    fn test_lokr_freeze_unfreeze() {
        let config = LoKrConfig::default();
        let device = Device::Cpu;
        let mut layer = LoKrLayer::new(64, 64, config, &device).unwrap();

        assert!(!layer.is_frozen());
        layer.freeze();
        assert!(layer.is_frozen());
        layer.unfreeze();
        assert!(!layer.is_frozen());
    }

    #[test]
    fn test_kronecker_product() {
        let device = Device::Cpu;
        let a = Tensor::new(&[[1.0f32, 2.0], [3.0, 4.0]], &device).unwrap();
        let b = Tensor::new(&[[0.0f32, 5.0], [6.0, 7.0]], &device).unwrap();

        let result = LoKrLayer::kronecker_product(&a, &b).unwrap();
        assert_eq!(result.dims(), &[4, 4]);
    }
}
